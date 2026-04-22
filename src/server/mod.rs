mod handler;

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub use self::handler::Handler;

use crate::{
    error::Error,
    protocol::{serialize_packet_into_buf, Packet, StatusCode},
    utils::read_packet_into_buf,
};

const PACKET_BUF_CAPACITY: usize = 32 * 1024;
const MAX_REUSABLE_WRITE_BUF_SIZE: usize = 256 * 1024;

fn reset_write_buf_if_oversized(write_buf: &mut BytesMut) {
    if write_buf.capacity() > MAX_REUSABLE_WRITE_BUF_SIZE {
        *write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
    } else {
        write_buf.clear();
    }
}

fn handle_to_string(handle: Bytes) -> String {
    String::from_utf8_lossy(&handle).into_owned()
}

macro_rules! into_wrap {
    ($id:expr, $handler:expr, $var:ident; $($arg:ident),*) => {
        match $handler.$var($($var.$arg),*).await {
            Err(err) => Packet::error($id, err.into()),
            Ok(packet) => packet.into(),
        }
    };
}

macro_rules! into_result {
    ($id:expr, $future:expr) => {
        match $future.await {
            Err(err) => Packet::error($id, err.into()),
            Ok(packet) => packet.into(),
        }
    };
}

async fn process_request<H>(packet: Packet, handler: &mut H) -> Packet
where
    H: Handler<String, Vec<u8>> + Send,
{
    let id = packet.get_request_id();

    match packet {
        Packet::Init(init) => into_wrap!(id, handler, init; version, extensions),
        Packet::Open(open) => into_wrap!(id, handler, open; id, filename, pflags, attrs),
        Packet::Close(close) => {
            into_result!(id, handler.close(close.id, handle_to_string(close.handle)))
        }
        Packet::Read(read) => into_result!(
            id,
            handler.read(
                read.id,
                handle_to_string(read.handle),
                read.offset,
                read.len
            )
        ),
        Packet::Write(write) => into_result!(
            id,
            handler.write(
                write.id,
                handle_to_string(write.handle),
                write.offset,
                write.data.to_vec()
            )
        ),
        Packet::Lstat(lstat) => into_wrap!(id, handler, lstat; id, path),
        Packet::Fstat(fstat) => {
            into_result!(id, handler.fstat(fstat.id, handle_to_string(fstat.handle)))
        }
        Packet::SetStat(setstat) => into_wrap!(id, handler, setstat; id, path, attrs),
        Packet::FSetStat(fsetstat) => into_result!(
            id,
            handler.fsetstat(
                fsetstat.id,
                handle_to_string(fsetstat.handle),
                fsetstat.attrs
            )
        ),
        Packet::OpenDir(opendir) => into_wrap!(id, handler, opendir; id, path),
        Packet::ReadDir(readdir) => {
            into_result!(
                id,
                handler.readdir(readdir.id, handle_to_string(readdir.handle))
            )
        }
        Packet::Remove(remove) => into_wrap!(id, handler, remove; id, filename),
        Packet::MkDir(mkdir) => into_wrap!(id, handler, mkdir; id, path, attrs),
        Packet::RmDir(rmdir) => into_wrap!(id, handler, rmdir; id, path),
        Packet::RealPath(realpath) => into_wrap!(id, handler, realpath; id, path),
        Packet::Stat(stat) => into_wrap!(id, handler, stat; id, path),
        Packet::Rename(rename) => into_wrap!(id, handler, rename; id, oldpath, newpath),
        Packet::ReadLink(readlink) => into_wrap!(id, handler, readlink; id, path),
        Packet::Symlink(symlink) => into_wrap!(id, handler, symlink; id, linkpath, targetpath),
        Packet::Extended(extended) => into_wrap!(id, handler, extended; id, request, data),
        _ => Packet::error(0, StatusCode::BadMessage),
    }
}

async fn process_request_bytes<H>(packet: Packet, handler: &mut H) -> Packet
where
    H: Handler<Bytes, Bytes> + Send,
{
    let id = packet.get_request_id();

    match packet {
        Packet::Init(init) => into_wrap!(id, handler, init; version, extensions),
        Packet::Open(open) => into_wrap!(id, handler, open; id, filename, pflags, attrs),
        Packet::Close(close) => into_wrap!(id, handler, close; id, handle),
        Packet::Read(read) => into_wrap!(id, handler, read; id, handle, offset, len),
        Packet::Write(write) => into_wrap!(id, handler, write; id, handle, offset, data),
        Packet::Lstat(lstat) => into_wrap!(id, handler, lstat; id, path),
        Packet::Fstat(fstat) => into_wrap!(id, handler, fstat; id, handle),
        Packet::SetStat(setstat) => into_wrap!(id, handler, setstat; id, path, attrs),
        Packet::FSetStat(fsetstat) => into_wrap!(id, handler, fsetstat; id, handle, attrs),
        Packet::OpenDir(opendir) => into_wrap!(id, handler, opendir; id, path),
        Packet::ReadDir(readdir) => into_wrap!(id, handler, readdir; id, handle),
        Packet::Remove(remove) => into_wrap!(id, handler, remove; id, filename),
        Packet::MkDir(mkdir) => into_wrap!(id, handler, mkdir; id, path, attrs),
        Packet::RmDir(rmdir) => into_wrap!(id, handler, rmdir; id, path),
        Packet::RealPath(realpath) => into_wrap!(id, handler, realpath; id, path),
        Packet::Stat(stat) => into_wrap!(id, handler, stat; id, path),
        Packet::Rename(rename) => into_wrap!(id, handler, rename; id, oldpath, newpath),
        Packet::ReadLink(readlink) => into_wrap!(id, handler, readlink; id, path),
        Packet::Symlink(symlink) => into_wrap!(id, handler, symlink; id, linkpath, targetpath),
        Packet::Extended(extended) => into_wrap!(id, handler, extended; id, request, data),
        _ => Packet::error(0, StatusCode::BadMessage),
    }
}

async fn process_handler<H, S>(
    stream: &mut S,
    handler: &mut H,
    read_buf: &mut BytesMut,
    write_buf: &mut BytesMut,
) -> Result<(), Error>
where
    H: Handler<String, Vec<u8>> + Send,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut packet_buf = read_packet_into_buf(stream, read_buf).await?;

    let response = match Packet::try_from(packet_buf.as_mut_bytes()) {
        Ok(request) => process_request(request, handler).await,
        Err(_) => Packet::error(0, StatusCode::BadMessage),
    };

    if let Err(err) = serialize_packet_into_buf(response, write_buf) {
        reset_write_buf_if_oversized(write_buf);
        return Err(err);
    }
    stream.write_all(write_buf).await?;
    stream.flush().await?;
    reset_write_buf_if_oversized(write_buf);

    Ok(())
}

async fn process_handler_bytes<H, S>(
    stream: &mut S,
    handler: &mut H,
    read_buf: &mut BytesMut,
    write_buf: &mut BytesMut,
) -> Result<(), Error>
where
    H: Handler<Bytes, Bytes> + Send,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut packet_buf = read_packet_into_buf(stream, read_buf).await?;

    let response = match Packet::try_from(packet_buf.as_mut_bytes()) {
        Ok(request) => process_request_bytes(request, handler).await,
        Err(_) => Packet::error(0, StatusCode::BadMessage),
    };

    if let Err(err) = serialize_packet_into_buf(response, write_buf) {
        reset_write_buf_if_oversized(write_buf);
        return Err(err);
    }
    stream.write_all(write_buf).await?;
    stream.flush().await?;
    reset_write_buf_if_oversized(write_buf);

    Ok(())
}

/// Run processing stream as SFTP using the backwards-compatible string-handle API.
pub async fn run<S, H>(mut stream: S, mut handler: H)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Handler<String, Vec<u8>> + Send + 'static,
{
    tokio::spawn(async move {
        // Reusable buffers for reading/writing packets - avoids allocation per packet.
        let mut read_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
        let mut write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);

        loop {
            match process_handler(&mut stream, &mut handler, &mut read_buf, &mut write_buf).await {
                Err(Error::UnexpectedEof) => break,
                Err(err) => warn!("{}", err),
                Ok(_) => (),
            }
        }

        debug!("sftp stream ended");
    });
}

/// Run processing stream as SFTP using opaque byte handles and write payloads.
pub async fn run_bytes<S, H>(mut stream: S, mut handler: H)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Handler<Bytes, Bytes> + Send + 'static,
{
    tokio::spawn(async move {
        let mut read_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
        let mut write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);

        loop {
            match process_handler_bytes(&mut stream, &mut handler, &mut read_buf, &mut write_buf)
                .await
            {
                Err(Error::UnexpectedEof) => break,
                Err(err) => warn!("{}", err),
                Ok(_) => (),
            }
        }

        debug!("sftp stream ended");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Status, Write};
    use std::future::Future;

    fn ok_status(id: u32) -> Status {
        Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        }
    }

    #[derive(Default)]
    struct LegacyHandler {
        handle: Option<String>,
        data: Option<Vec<u8>>,
    }

    impl Handler for LegacyHandler {
        type Error = StatusCode;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        fn write(
            &mut self,
            id: u32,
            handle: String,
            _offset: u64,
            data: Vec<u8>,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
            self.handle = Some(handle);
            self.data = Some(data);
            async move { Ok(ok_status(id)) }
        }
    }

    #[derive(Default)]
    struct BytesHandler {
        handle: Option<Bytes>,
        data: Option<Bytes>,
    }

    impl Handler<Bytes, Bytes> for BytesHandler {
        type Error = StatusCode;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        fn write(
            &mut self,
            id: u32,
            handle: Bytes,
            _offset: u64,
            data: Bytes,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
            self.handle = Some(handle);
            self.data = Some(data);
            async move { Ok(ok_status(id)) }
        }
    }

    #[tokio::test]
    async fn default_run_path_preserves_string_vec_handler_api() {
        let mut handler = LegacyHandler::default();
        let response = process_request(
            Packet::Write(Write::from_string_vec(
                7,
                "legacy-handle",
                0,
                b"legacy-data".to_vec(),
            )),
            &mut handler,
        )
        .await;

        assert!(matches!(
            response,
            Packet::Status(Status {
                status_code: StatusCode::Ok,
                ..
            })
        ));
        assert_eq!(handler.handle.as_deref(), Some("legacy-handle"));
        assert_eq!(handler.data.as_deref(), Some(&b"legacy-data"[..]));
    }

    #[tokio::test]
    async fn bytes_run_path_preserves_bytes_handler_api() {
        let mut handler = BytesHandler::default();
        let response = process_request_bytes(
            Packet::Write(Write::new(
                8,
                Bytes::from_static(b"bytes-handle"),
                0,
                Bytes::from_static(b"bytes-data"),
            )),
            &mut handler,
        )
        .await;

        assert!(matches!(
            response,
            Packet::Status(Status {
                status_code: StatusCode::Ok,
                ..
            })
        ));
        assert_eq!(
            handler.handle.as_ref().map(Bytes::as_ref),
            Some(&b"bytes-handle"[..])
        );
        assert_eq!(
            handler.data.as_ref().map(Bytes::as_ref),
            Some(&b"bytes-data"[..])
        );
    }
}
