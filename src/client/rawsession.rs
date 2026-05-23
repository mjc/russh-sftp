use bytes::Bytes;
use dashmap::DashMap;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, RwLock},
    time,
};

use super::{error::Error, run, Handler};
use crate::{
    de,
    extensions::{
        self, FsyncExtension, HardlinkExtension, LimitsExtension, Statvfs, StatvfsExtension,
    },
    protocol::{
        serialize_read_packet, serialize_write_packet, Attrs, Close, Data, Extended, ExtendedReply,
        FSetStat, FileAttributes, Fstat, Handle, Init, Lstat, MkDir, Name, Open, OpenDir,
        OpenFlags, Packet, ReadDir, ReadLink, RealPath, Remove, Rename, RmDir, SetStat, Stat,
        Status, StatusCode, Symlink, Version,
    },
};

pub type SftpResult<T> = Result<T, Error>;
type SharedRequests = DashMap<Option<u32>, oneshot::Sender<SftpResult<Packet>>>;

pub(crate) struct SessionInner {
    version: Option<u32>,
    requests: Arc<SharedRequests>,
}

impl SessionInner {
    pub async fn reply(&mut self, id: Option<u32>, packet: Packet) -> SftpResult<()> {
        if let Some((_, sender)) = self.requests.remove(&id) {
            let validate = if id.is_some() && self.version.is_none() {
                Err(Error::UnexpectedPacket)
            } else if id.is_none() && self.version.is_some() {
                Err(Error::UnexpectedBehavior("Duplicate version".to_owned()))
            } else {
                Ok(())
            };

            sender
                .send(validate.clone().map(|_| packet))
                .map_err(|_| Error::UnexpectedBehavior("request receiver dropped".to_owned()))?;

            return validate;
        }

        Err(Error::UnexpectedBehavior(format!(
            "Packet {:?} for unknown recipient",
            id
        )))
    }
}

#[allow(clippy::manual_async_fn)]
impl Handler for SessionInner {
    type Error = Error;

    fn version(&mut self, packet: Version) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move {
            let version = packet.version;
            self.reply(None, packet.into()).await?;
            self.version = Some(version);
            Ok(())
        }
    }

    fn name(&mut self, name: Name) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(name.id), name.into()).await }
    }

    fn status(&mut self, status: Status) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(status.id), status.into()).await }
    }

    fn handle(&mut self, handle: Handle) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(handle.id), handle.into()).await }
    }

    fn data(&mut self, data: Data) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(data.id), data.into()).await }
    }

    fn attrs(&mut self, attrs: Attrs) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(attrs.id), attrs.into()).await }
    }

    fn extended_reply(
        &mut self,
        reply: ExtendedReply,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move { self.reply(Some(reply.id), reply.into()).await }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    // todo: implement
    //pub packet_len: Option<u64>,
    pub read_len: Option<u64>,
    pub write_len: Option<u64>,
    pub open_handles: Option<u64>,
}

impl From<LimitsExtension> for Limits {
    fn from(limits: LimitsExtension) -> Self {
        Self {
            read_len: if limits.max_read_len > 0 {
                Some(limits.max_read_len)
            } else {
                None
            },
            write_len: if limits.max_write_len > 0 {
                Some(limits.max_write_len)
            } else {
                None
            },
            open_handles: if limits.max_open_handles > 0 {
                Some(limits.max_open_handles)
            } else {
                None
            },
        }
    }
}

pub(crate) struct Options {
    timeout: RwLock<u64>,
    limits: Arc<Limits>,
}

/// Implements raw work with the protocol in request-response format.
/// If the server returns a `Status` packet and it has the code Ok
/// then the packet is returned as Ok in other error cases
/// the packet is stored as Err.
pub struct RawSftpSession {
    tx: mpsc::UnboundedSender<Bytes>,
    requests: Arc<SharedRequests>,
    next_req_id: AtomicU32,
    handles: AtomicU64,
    options: Options,
}

macro_rules! into_with_status {
    ($result:ident, $packet:ident) => {
        match $result {
            Packet::$packet(p) => Ok(p),
            Packet::Status(p) => Err(p.into()),
            _ => Err(Error::UnexpectedPacket),
        }
    };
}

macro_rules! into_status {
    ($result:ident) => {
        match $result {
            Packet::Status(status) if status.status_code == StatusCode::Ok => Ok(status),
            Packet::Status(status) => Err(status.into()),
            _ => Err(Error::UnexpectedPacket),
        }
    };
}

impl RawSftpSession {
    pub fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let req_map = Arc::new(DashMap::new());
        let inner = SessionInner {
            version: None,
            requests: req_map.clone(),
        };

        Self {
            tx: run(stream, inner),
            requests: req_map,
            next_req_id: AtomicU32::new(1),
            handles: AtomicU64::new(0),
            options: Options {
                timeout: RwLock::new(10),
                limits: Arc::new(Limits::default()),
            },
        }
    }

    /// Set the maximum response time in seconds.
    /// Default: 10 seconds. A value of 0 disables request timeouts.
    pub async fn set_timeout(&self, secs: u64) {
        *self.options.timeout.write().await = secs;
    }

    /// Setting limits. For the `limits@openssh.com` extension
    pub fn set_limits(&mut self, limits: Arc<Limits>) {
        self.options.limits = limits;
    }

    async fn send(&self, id: Option<u32>, packet: Packet) -> SftpResult<Packet> {
        let bytes = Bytes::try_from(packet)?;
        self.send_bytes(id, bytes).await
    }

    async fn send_bytes(&self, id: Option<u32>, bytes: Bytes) -> SftpResult<Packet> {
        if self.tx.is_closed() {
            return Err(Error::UnexpectedBehavior("session closed".into()));
        }

        let (tx, rx) = oneshot::channel();

        self.requests.insert(id, tx);
        if let Err(err) = self.tx.send(bytes) {
            self.requests.remove(&id);
            return Err(err.into());
        }

        let timeout = *self.options.timeout.read().await;
        if timeout == 0 {
            return rx
                .await
                .map_err(|_| Error::UnexpectedBehavior("request receiver dropped".into()))?;
        }

        match time::timeout(Duration::from_secs(timeout), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::UnexpectedBehavior("request receiver dropped".into())),
            Err(error) => {
                self.requests.remove(&id);
                Err(error.into())
            }
        }
    }

    fn use_next_id(&self) -> u32 {
        self.next_req_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Closes the inner channel stream. Called by [`Drop`]
    pub fn close_session(&self) -> SftpResult<()> {
        if self.tx.is_closed() {
            return Ok(());
        }

        Ok(self.tx.send(Bytes::new())?)
    }

    pub async fn init(&self) -> SftpResult<Version> {
        let result = self.send(None, Init::default().into()).await?;
        if let Packet::Version(version) = result {
            Ok(version)
        } else {
            Err(Error::UnexpectedPacket)
        }
    }

    pub async fn open<T: Into<String>>(
        &self,
        filename: T,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> SftpResult<Handle> {
        if self
            .options
            .limits
            .open_handles
            .is_some_and(|h| self.handles.load(Ordering::SeqCst) >= h)
        {
            return Err(Error::Limited("handle limit reached".to_owned()));
        }

        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Open {
                    id,
                    filename: filename.into(),
                    pflags: flags,
                    attrs,
                }
                .into(),
            )
            .await?;

        if let Packet::Handle(_) = result {
            self.handles.fetch_add(1, Ordering::SeqCst);
        }

        into_with_status!(result, Handle)
    }

    pub async fn close<H: Into<String>>(&self, handle: H) -> SftpResult<Status> {
        self.close_bytes(Bytes::from(handle.into())).await
    }

    pub async fn close_bytes(&self, handle: Bytes) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self.send(Some(id), Close { id, handle }.into()).await?;

        if let Packet::Status(status) = &result {
            if status.status_code == StatusCode::Ok
                && self
                    .handles
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |h| {
                        if h > 0 {
                            Some(h - 1)
                        } else {
                            None
                        }
                    })
                    .is_err()
            {
                warn!("attempt to close more handles than exist");
            }
        }

        into_status!(result)
    }

    pub async fn read<H: Into<String>>(
        &self,
        handle: H,
        offset: u64,
        len: u32,
    ) -> SftpResult<Data> {
        self.read_bytes(Bytes::from(handle.into()), offset, len)
            .await
    }

    pub async fn read_bytes(&self, handle: Bytes, offset: u64, len: u32) -> SftpResult<Data> {
        if self.options.limits.read_len.is_some_and(|r| len as u64 > r) {
            return Err(Error::Limited("read limit reached".to_owned()));
        }

        let id = self.use_next_id();
        let bytes = serialize_read_packet(id, &handle, offset, len)?;
        let result = self.send_bytes(Some(id), bytes).await?;

        into_with_status!(result, Data)
    }

    pub async fn write<H: Into<String>>(
        &self,
        handle: H,
        offset: u64,
        data: Vec<u8>,
    ) -> SftpResult<Status> {
        self.write_bytes(Bytes::from(handle.into()), offset, data.into())
            .await
    }

    pub async fn write_bytes(&self, handle: Bytes, offset: u64, data: Bytes) -> SftpResult<Status> {
        if self
            .options
            .limits
            .write_len
            .is_some_and(|w| data.len() as u64 > w)
        {
            return Err(Error::Limited("write limit reached".to_owned()));
        }

        let id = self.use_next_id();
        let bytes = serialize_write_packet(id, &handle, offset, &data)?;
        let result = self.send_bytes(Some(id), bytes).await?;

        into_status!(result)
    }

    pub async fn lstat<P: Into<String>>(&self, path: P) -> SftpResult<Attrs> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Lstat {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        into_with_status!(result, Attrs)
    }

    pub async fn fstat<H: Into<String>>(&self, handle: H) -> SftpResult<Attrs> {
        self.fstat_bytes(Bytes::from(handle.into())).await
    }

    pub async fn fstat_bytes(&self, handle: Bytes) -> SftpResult<Attrs> {
        let id = self.use_next_id();
        let result = self.send(Some(id), Fstat { id, handle }.into()).await?;

        into_with_status!(result, Attrs)
    }

    pub async fn setstat<P: Into<String>>(
        &self,
        path: P,
        attrs: FileAttributes,
    ) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                SetStat {
                    id,
                    path: path.into(),
                    attrs,
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    pub async fn fsetstat<H: Into<String>>(
        &self,
        handle: H,
        attrs: FileAttributes,
    ) -> SftpResult<Status> {
        self.fsetstat_bytes(Bytes::from(handle.into()), attrs).await
    }

    pub async fn fsetstat_bytes(&self, handle: Bytes, attrs: FileAttributes) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self
            .send(Some(id), FSetStat { id, handle, attrs }.into())
            .await?;

        into_status!(result)
    }

    pub async fn opendir<P: Into<String>>(&self, path: P) -> SftpResult<Handle> {
        if self
            .options
            .limits
            .open_handles
            .is_some_and(|h| self.handles.load(Ordering::SeqCst) >= h)
        {
            return Err(Error::Limited("Handle limit reached".to_owned()));
        }

        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                OpenDir {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        if let Packet::Handle(_) = result {
            self.handles.fetch_add(1, Ordering::SeqCst);
        }

        into_with_status!(result, Handle)
    }

    pub async fn readdir<H: Into<String>>(&self, handle: H) -> SftpResult<Name> {
        self.readdir_bytes(Bytes::from(handle.into())).await
    }

    pub async fn readdir_bytes(&self, handle: Bytes) -> SftpResult<Name> {
        let id = self.use_next_id();
        let result = self.send(Some(id), ReadDir { id, handle }.into()).await?;

        into_with_status!(result, Name)
    }

    pub async fn remove<T: Into<String>>(&self, filename: T) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Remove {
                    id,
                    filename: filename.into(),
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    pub async fn mkdir<P: Into<String>>(
        &self,
        path: P,
        attrs: FileAttributes,
    ) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                MkDir {
                    id,
                    path: path.into(),
                    attrs,
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    pub async fn rmdir<P: Into<String>>(&self, path: P) -> SftpResult<Status> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                RmDir {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    pub async fn realpath<P: Into<String>>(&self, path: P) -> SftpResult<Name> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                RealPath {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        into_with_status!(result, Name)
    }

    pub async fn stat<P: Into<String>>(&self, path: P) -> SftpResult<Attrs> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Stat {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        into_with_status!(result, Attrs)
    }

    pub async fn rename<O, N>(&self, oldpath: O, newpath: N) -> SftpResult<Status>
    where
        O: Into<String>,
        N: Into<String>,
    {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Rename {
                    id,
                    oldpath: oldpath.into(),
                    newpath: newpath.into(),
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    pub async fn readlink<P: Into<String>>(&self, path: P) -> SftpResult<Name> {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                ReadLink {
                    id,
                    path: path.into(),
                }
                .into(),
            )
            .await?;

        into_with_status!(result, Name)
    }

    pub async fn symlink<P, T>(&self, path: P, target: T) -> SftpResult<Status>
    where
        P: Into<String>,
        T: Into<String>,
    {
        let id = self.use_next_id();
        let result = self
            .send(
                Some(id),
                Symlink {
                    id,
                    linkpath: path.into(),
                    targetpath: target.into(),
                }
                .into(),
            )
            .await?;

        into_status!(result)
    }

    /// Equivalent to `SSH_FXP_EXTENDED`. Allows protocol expansion.
    /// The extension can return any packet, so it's not specific
    pub async fn extended<R: Into<String>>(&self, request: R, data: Vec<u8>) -> SftpResult<Packet> {
        let id = self.use_next_id();
        self.send(
            Some(id),
            Extended {
                id,
                request: request.into(),
                data,
            }
            .into(),
        )
        .await
    }

    pub async fn limits(&self) -> SftpResult<LimitsExtension> {
        match self.extended(extensions::LIMITS, vec![]).await? {
            Packet::ExtendedReply(reply) => {
                Ok(de::from_bytes::<LimitsExtension>(&mut reply.data.into())?)
            }
            Packet::Status(status) if status.status_code != StatusCode::Ok => {
                Err(Error::Status(status))
            }
            _ => Err(Error::UnexpectedPacket),
        }
    }

    pub async fn hardlink<O, N>(&self, oldpath: O, newpath: N) -> SftpResult<Status>
    where
        O: Into<String>,
        N: Into<String>,
    {
        let result = self
            .extended(
                extensions::HARDLINK,
                HardlinkExtension {
                    oldpath: oldpath.into(),
                    newpath: newpath.into(),
                }
                .try_into()?,
            )
            .await?;

        into_status!(result)
    }

    pub async fn fsync<H: Into<String>>(&self, handle: H) -> SftpResult<Status> {
        self.fsync_bytes(Bytes::from(handle.into())).await
    }

    pub async fn fsync_bytes(&self, handle: Bytes) -> SftpResult<Status> {
        let result = self
            .extended(extensions::FSYNC, FsyncExtension { handle }.try_into()?)
            .await?;

        into_status!(result)
    }

    pub async fn statvfs<P>(&self, path: P) -> SftpResult<Statvfs>
    where
        P: Into<String>,
    {
        let result = self
            .extended(
                extensions::STATVFS,
                StatvfsExtension { path: path.into() }.try_into()?,
            )
            .await?;

        match result {
            Packet::ExtendedReply(reply) => Ok(de::from_bytes::<Statvfs>(&mut reply.data.into())?),
            Packet::Status(status) if status.status_code != StatusCode::Ok => {
                Err(Error::Status(status))
            }
            _ => Err(Error::UnexpectedPacket),
        }
    }
}

impl Drop for RawSftpSession {
    fn drop(&mut self) {
        let _ = self.close_session();
    }
}
