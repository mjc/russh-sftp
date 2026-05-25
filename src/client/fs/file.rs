use std::{
    collections::BTreeMap,
    future::Future,
    io::{self, SeekFrom},
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};

use bytes::{Buf, Bytes};
use futures::{
    stream::{FuturesUnordered, Stream},
    FutureExt,
};
use tokio::{
    io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf},
    runtime::Handle,
};

use super::Metadata;
use crate::{
    client::{error::Error, rawsession::RawSftpSession, session::Extensions, SftpResult},
    protocol::StatusCode,
};

type StateFn<T> = Option<Pin<Box<dyn Future<Output = io::Result<T>> + Send + Sync + 'static>>>;
type ReadFuture = Pin<Box<dyn Future<Output = io::Result<ReadResult>> + Send + 'static>>;

const MAX_READ_LENGTH: u64 = 261120;
const MAX_WRITE_LENGTH: u64 = 261120;
const DEFAULT_READ_DEPTH: usize = 64;

struct ReadResult {
    offset: u64,
    len: u32,
    data: Option<Bytes>,
}

struct FileState {
    read_pending: FuturesUnordered<ReadFuture>,
    read_ready: BTreeMap<u64, Bytes>,
    read_next_offset: u64,
    read_eof: Option<u64>,
    read_depth: usize,
    f_seek: StateFn<u64>,
    f_write: StateFn<usize>,
    f_flush: StateFn<()>,
    f_shutdown: StateFn<()>,
}

impl FileState {
    fn reset_read(&mut self, offset: u64) {
        self.read_pending = FuturesUnordered::new();
        self.read_ready.clear();
        self.read_next_offset = offset;
        self.read_eof = None;
    }
}

/// Provides high-level methods for interaction with a remote file.
///
/// In order to properly close the handle, [`shutdown`] on a file should be called.
/// Also implement [`AsyncSeek`] and other async i/o implementations.
///
/// # Weakness
/// Using [`SeekFrom::End`] is costly and time-consuming because we need to
/// request the actual file size from the remote server.
pub struct File {
    session: Arc<RawSftpSession>,
    handle: Bytes,
    state: FileState,
    pos: u64,
    closed: bool,
    extensions: Arc<Extensions>,
}

impl File {
    pub(crate) fn new(
        session: Arc<RawSftpSession>,
        handle: Bytes,
        extensions: Arc<Extensions>,
    ) -> Self {
        Self {
            session,
            handle,
            state: FileState {
                read_pending: FuturesUnordered::new(),
                read_ready: BTreeMap::new(),
                read_next_offset: 0,
                read_eof: None,
                read_depth: DEFAULT_READ_DEPTH,
                f_seek: None,
                f_write: None,
                f_flush: None,
                f_shutdown: None,
            },
            pos: 0,
            closed: false,
            extensions,
        }
    }

    /// Queries metadata about the remote file.
    pub async fn metadata(&self) -> SftpResult<Metadata> {
        Ok(self.session.fstat_bytes(self.handle.clone()).await?.attrs)
    }

    /// Sets metadata for a remote file.
    pub async fn set_metadata(&self, metadata: Metadata) -> SftpResult<()> {
        self.session
            .fsetstat_bytes(self.handle.clone(), metadata)
            .await
            .map(|_| ())
    }

    /// Attempts to sync all data.
    ///
    /// If the server does not support `fsync@openssh.com` sending the request will
    /// be omitted, but will still pseudo-successfully
    pub async fn sync_all(&self) -> SftpResult<()> {
        if !self.extensions.fsync {
            return Ok(());
        }

        self.session
            .fsync_bytes(self.handle.clone())
            .await
            .map(|_| ())
    }

    /// Reads bytes from a specific offset without changing the file cursor.
    pub async fn read_at(&self, offset: u64, len: u32) -> SftpResult<Bytes> {
        self.session
            .read_bytes(self.handle.clone(), offset, len)
            .await
            .map(|data| data.data.into_bytes())
    }

    /// Writes bytes at a specific offset without changing the file cursor.
    pub async fn write_at(&self, offset: u64, data: Bytes) -> SftpResult<()> {
        self.session
            .write_bytes(self.handle.clone(), offset, data)
            .await
            .map(|_| ())
    }

    /// Closes the remote file handle.
    pub async fn close(mut self) -> SftpResult<()> {
        if self.closed {
            return Ok(());
        }

        self.closed = true;
        self.session
            .close_bytes(self.handle.clone())
            .await
            .map(|_| ())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if self.closed {
            return;
        }

        if let Ok(handle) = Handle::try_current() {
            let session = self.session.clone();
            let file_handle = self.handle.clone();

            handle.spawn(async move {
                let _ = session.close_bytes(file_handle).await;
            });
        }
    }
}

impl AsyncRead for File {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        if self.state.read_pending.is_empty()
            && self.state.read_ready.is_empty()
            && self.state.read_next_offset != self.pos
        {
            let pos = self.pos;
            self.state.reset_read(pos);
        }

        let pos = self.pos;
        if let Some(mut data) = self.state.read_ready.remove(&pos) {
            let len = data.len().min(buf.remaining());
            buf.put_slice(&data[..len]);
            self.pos += len as u64;

            if len < data.len() {
                data.advance(len);
                let pos = self.pos;
                self.state.read_ready.insert(pos, data);
            }

            return Poll::Ready(Ok(()));
        }

        let max_read_len = self
            .extensions
            .limits
            .as_ref()
            .and_then(|l| l.read_len)
            .unwrap_or(MAX_READ_LENGTH) as usize;

        while self.state.read_eof.is_none() && self.state.read_pending.len() < self.state.read_depth
        {
            let session = self.session.clone();
            let file_handle = self.handle.clone();
            let offset = self.state.read_next_offset;
            let len = max_read_len.min(u32::MAX as usize) as u32;

            self.state.read_next_offset += u64::from(len);
            self.state.read_pending.push(
                async move {
                    let result = session.read_bytes(file_handle, offset, len).await;

                    match result {
                        Ok(data) => Ok(ReadResult {
                            offset,
                            len,
                            data: Some(data.data.into_bytes()),
                        }),
                        Err(Error::Status(status)) if status.status_code == StatusCode::Eof => {
                            Ok(ReadResult {
                                offset,
                                len,
                                data: None,
                            })
                        }
                        Err(e) => Err(io::Error::other(e.to_string())),
                    }
                }
                .boxed(),
            );
        }

        loop {
            match Pin::new(&mut self.state.read_pending).poll_next(cx) {
                Poll::Pending => {
                    if self.state.read_eof == Some(self.pos) {
                        return Poll::Ready(Ok(()));
                    }
                    return Poll::Pending;
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Ok(()));
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e)),
                Poll::Ready(Some(Ok(result))) => {
                    match result.data {
                        Some(data) if data.is_empty() => {
                            self.state.read_eof.get_or_insert(result.offset);
                        }
                        Some(data) => {
                            if data.len() < result.len as usize {
                                self.state
                                    .read_eof
                                    .get_or_insert(result.offset + data.len() as u64);
                            }
                            if self.state.read_eof.is_none_or(|eof| result.offset < eof) {
                                self.state.read_ready.insert(result.offset, data);
                            }
                        }
                        None => {
                            self.state.read_eof.get_or_insert(result.offset);
                        }
                    }

                    let pos = self.pos;
                    if let Some(mut data) = self.state.read_ready.remove(&pos) {
                        let len = data.len().min(buf.remaining());
                        buf.put_slice(&data[..len]);
                        self.pos += len as u64;

                        if len < data.len() {
                            data.advance(len);
                            let pos = self.pos;
                            self.state.read_ready.insert(pos, data);
                        }

                        return Poll::Ready(Ok(()));
                    }

                    if self.state.read_eof == Some(self.pos) && self.state.read_pending.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                }
            }
        }
    }
}

impl AsyncSeek for File {
    fn start_seek(mut self: Pin<&mut Self>, position: io::SeekFrom) -> io::Result<()> {
        if !self.state.read_pending.is_empty() {
            return Err(io::Error::other(
                "read operation is pending, poll it before start_seek",
            ));
        }

        match self.state.f_seek {
            Some(_) => Err(io::Error::other(
                "other file operation is pending, call poll_complete before start_seek",
            )),
            None => {
                let session = self.session.clone();
                let file_handle = self.handle.clone();
                let cur_pos = self.pos as i64;

                self.state.f_seek = Some(Box::pin(async move {
                    let new_pos = match position {
                        SeekFrom::Start(pos) => pos as i64,
                        SeekFrom::Current(pos) => cur_pos + pos,
                        SeekFrom::End(pos) => {
                            let result = session
                                .fstat_bytes(file_handle)
                                .await
                                .map_err(|e| io::Error::other(e.to_string()))?;

                            match result.attrs.size {
                                Some(size) => size as i64 + pos,
                                None => return Err(io::Error::other("file size unknown")),
                            }
                        }
                    };

                    if new_pos < 0 {
                        return Err(io::Error::other(
                            "cannot move file pointer before the beginning",
                        ));
                    }

                    Ok(new_pos as u64)
                }));

                Ok(())
            }
        }
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        match self.state.f_seek.as_mut() {
            None => Poll::Ready(Ok(self.pos)),
            Some(f) => {
                self.pos = ready!(Pin::new(f).poll(cx))?;
                self.state.f_seek = None;
                let pos = self.pos;
                self.state.reset_read(pos);
                Poll::Ready(Ok(self.pos))
            }
        }
    }
}

impl AsyncWrite for File {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let poll = Pin::new(match self.state.f_write.as_mut() {
            Some(f) => f,
            None => {
                let session = self.session.clone();
                let max_write_len = self
                    .extensions
                    .limits
                    .as_ref()
                    .and_then(|l| l.write_len)
                    .unwrap_or(MAX_WRITE_LENGTH) as usize;

                let file_handle = self.handle.clone();
                let offset = self.pos;
                let len = buf.len().min(max_write_len);
                let data = buf[..len].to_vec();

                self.state.f_write.get_or_insert(Box::pin(async move {
                    session
                        .write_bytes(file_handle, offset, data.into())
                        .await
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    Ok(len)
                }))
            }
        })
        .poll(cx);

        if poll.is_ready() {
            self.state.f_write = None;
        }

        if let Poll::Ready(Ok(len)) = poll {
            self.pos += len as u64;
        }

        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        if !self.extensions.fsync {
            return Poll::Ready(Ok(()));
        }

        let poll = Pin::new(match self.state.f_flush.as_mut() {
            Some(f) => f,
            None => {
                let session = self.session.clone();
                let file_handle = self.handle.clone();

                self.state.f_flush.get_or_insert(Box::pin(async move {
                    session
                        .fsync_bytes(file_handle)
                        .await
                        .map(|_| ())
                        .map_err(|e| io::Error::other(e.to_string()))
                }))
            }
        })
        .poll(cx);

        if poll.is_ready() {
            self.state.f_flush = None;
        }

        poll
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let poll = Pin::new(match self.state.f_shutdown.as_mut() {
            Some(f) => f,
            None => {
                let session = self.session.clone();
                let file_handle = self.handle.clone();

                self.state.f_shutdown.get_or_insert(Box::pin(async move {
                    session
                        .close_bytes(file_handle)
                        .await
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    Ok(())
                }))
            }
        })
        .poll(cx);

        if poll.is_ready() {
            self.state.f_shutdown = None;
            self.closed = true;
        }

        poll
    }
}
