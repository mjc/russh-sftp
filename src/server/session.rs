use std::{collections::HashMap, future::Future};

use bytes::Bytes;

use super::{Handler, SessionHandles};
use crate::protocol::{
    Attrs, Data, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};

async fn unsupported<T, E>(err: E) -> Result<T, E> {
    Err(err)
}

pub trait SessionHandler: Sized {
    type Error: Into<StatusCode> + Send;
    type File: Send;
    type Dir: Send;

    fn unimplemented(&self) -> Self::Error;

    fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> impl Future<Output = Result<Version, Self::Error>> + Send {
        async { Ok(Version::new()) }
    }

    fn open(
        &mut self,
        _id: u32,
        _filename: String,
        _pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Self::File, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn opendir(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Self::Dir, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn close_file(
        &mut self,
        id: u32,
        _file: Self::File,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        async move { Ok(ok_status(id)) }
    }

    fn close_dir(
        &mut self,
        id: u32,
        _dir: Self::Dir,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        async move { Ok(ok_status(id)) }
    }

    fn read<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
        _offset: u64,
        _len: u32,
    ) -> impl Future<Output = Result<Data, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn write<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
        _offset: u64,
        _data: Bytes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn readdir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn fstat_file<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn fstat_dir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn fsetstat_file<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn fsetstat_dir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        unsupported(self.unimplemented())
    }

    fn lstat(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn setstat(
        &mut self,
        _id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn remove(
        &mut self,
        _id: u32,
        _filename: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn mkdir(
        &mut self,
        _id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn rmdir(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn realpath(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn stat(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn rename(
        &mut self,
        _id: u32,
        _oldpath: String,
        _newpath: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn readlink(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn symlink(
        &mut self,
        _id: u32,
        _linkpath: String,
        _targetpath: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }

    fn extended(
        &mut self,
        _id: u32,
        _request: String,
        _data: Vec<u8>,
    ) -> impl Future<Output = Result<Packet, Self::Error>> + Send {
        unsupported(self.unimplemented())
    }
}

pub struct ManagedSession<H: SessionHandler> {
    handler: H,
    handles: SessionHandles<H::File, H::Dir>,
}

impl<H: SessionHandler> ManagedSession<H> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            handles: SessionHandles::new(),
        }
    }

    pub fn into_inner(self) -> H {
        self.handler
    }
}

impl<H> Handler for ManagedSession<H>
where
    H: SessionHandler + Send,
{
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        self.handler
            .init(version, extensions)
            .await
            .map_err(Into::into)
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let file = self
            .handler
            .open(id, filename, pflags, attrs)
            .await
            .map_err(Into::into)?;
        Ok(Handle {
            id,
            handle: self.handles.insert_file(file).into_bytes(),
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let dir = self.handler.opendir(id, path).await.map_err(Into::into)?;
        Ok(Handle {
            id,
            handle: self.handles.insert_dir(dir).into_bytes(),
        })
    }

    async fn close(&mut self, id: u32, handle: Bytes) -> Result<Status, Self::Error> {
        if let Some(file) = self.handles.remove_file_bytes(&handle) {
            return self.handler.close_file(id, file).await.map_err(Into::into);
        }
        if let Some(dir) = self.handles.remove_dir_bytes(&handle) {
            return self.handler.close_dir(id, dir).await.map_err(Into::into);
        }
        Err(StatusCode::Failure)
    }

    async fn read(
        &mut self,
        id: u32,
        handle: Bytes,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let file = self
            .handles
            .get_file_mut_bytes(&handle)
            .ok_or(StatusCode::Failure)?;
        self.handler
            .read(id, file, offset, len)
            .await
            .map_err(Into::into)
    }

    async fn write(
        &mut self,
        id: u32,
        handle: Bytes,
        offset: u64,
        data: Bytes,
    ) -> Result<Status, Self::Error> {
        let file = self
            .handles
            .get_file_mut_bytes(&handle)
            .ok_or(StatusCode::Failure)?;
        self.handler
            .write(id, file, offset, data)
            .await
            .map_err(Into::into)
    }

    async fn readdir(&mut self, id: u32, handle: Bytes) -> Result<Name, Self::Error> {
        let dir = self
            .handles
            .get_dir_mut_bytes(&handle)
            .ok_or(StatusCode::Failure)?;
        self.handler.readdir(id, dir).await.map_err(Into::into)
    }

    async fn fstat(&mut self, id: u32, handle: Bytes) -> Result<Attrs, Self::Error> {
        if let Some(file) = self.handles.get_file_mut_bytes(&handle) {
            return self.handler.fstat_file(id, file).await.map_err(Into::into);
        }
        if let Some(dir) = self.handles.get_dir_mut_bytes(&handle) {
            return self.handler.fstat_dir(id, dir).await.map_err(Into::into);
        }
        Err(StatusCode::Failure)
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        handle: Bytes,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        if let Some(file) = self.handles.get_file_mut_bytes(&handle) {
            return self
                .handler
                .fsetstat_file(id, file, attrs)
                .await
                .map_err(Into::into);
        }
        if let Some(dir) = self.handles.get_dir_mut_bytes(&handle) {
            return self
                .handler
                .fsetstat_dir(id, dir, attrs)
                .await
                .map_err(Into::into);
        }
        Err(StatusCode::Failure)
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.handler.lstat(id, path).await.map_err(Into::into)
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        self.handler
            .setstat(id, path, attrs)
            .await
            .map_err(Into::into)
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        self.handler.remove(id, filename).await.map_err(Into::into)
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        self.handler
            .mkdir(id, path, attrs)
            .await
            .map_err(Into::into)
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        self.handler.rmdir(id, path).await.map_err(Into::into)
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        self.handler.realpath(id, path).await.map_err(Into::into)
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.handler.stat(id, path).await.map_err(Into::into)
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        self.handler
            .rename(id, oldpath, newpath)
            .await
            .map_err(Into::into)
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        self.handler.readlink(id, path).await.map_err(Into::into)
    }

    async fn symlink(
        &mut self,
        id: u32,
        linkpath: String,
        targetpath: String,
    ) -> Result<Status, Self::Error> {
        self.handler
            .symlink(id, linkpath, targetpath)
            .await
            .map_err(Into::into)
    }

    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<Packet, Self::Error> {
        self.handler
            .extended(id, request, data)
            .await
            .map_err(Into::into)
    }
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".into(),
        language_tag: "en".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestHandler {
        reads: usize,
        readdirs: usize,
        closed_files: Vec<String>,
    }

    impl SessionHandler for TestHandler {
        type Error = StatusCode;
        type File = String;
        type Dir = String;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        fn open(
            &mut self,
            _id: u32,
            filename: String,
            _pflags: OpenFlags,
            _attrs: FileAttributes,
        ) -> impl Future<Output = Result<Self::File, Self::Error>> + Send {
            async move { Ok(filename) }
        }

        fn opendir(
            &mut self,
            _id: u32,
            path: String,
        ) -> impl Future<Output = Result<Self::Dir, Self::Error>> + Send {
            async move { Ok(path) }
        }

        fn close_file(
            &mut self,
            id: u32,
            file: Self::File,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
            self.closed_files.push(file);
            async move { Ok(ok_status(id)) }
        }

        fn read<'a>(
            &'a mut self,
            id: u32,
            file: &'a mut Self::File,
            offset: u64,
            len: u32,
        ) -> impl Future<Output = Result<Data, Self::Error>> + Send + 'a {
            self.reads += 1;
            async move {
                Ok(Data {
                    id,
                    data: Bytes::from(format!("{file}:{offset}:{len}")).into(),
                })
            }
        }

        fn readdir<'a>(
            &'a mut self,
            id: u32,
            dir: &'a mut Self::Dir,
        ) -> impl Future<Output = Result<Name, Self::Error>> + Send + 'a {
            self.readdirs += 1;
            async move {
                Ok(Name {
                    id,
                    files: vec![crate::protocol::File::dummy(dir.clone())],
                })
            }
        }
    }

    async fn handles(session: &mut ManagedSession<TestHandler>) -> (Bytes, Bytes) {
        let file = Handler::open(
            session,
            1,
            "file.txt".into(),
            OpenFlags::READ,
            FileAttributes::empty(),
        )
        .await
        .unwrap()
        .handle;
        let dir = Handler::opendir(session, 1, "/tmp".into())
            .await
            .unwrap()
            .handle;
        (file, dir)
    }

    #[tokio::test]
    async fn dispatches_typed_file_and_dir_handles() {
        let mut session = ManagedSession::new(TestHandler::default());
        let (file, dir) = handles(&mut session).await;

        assert_eq!(
            Handler::read(&mut session, 2, file.clone(), 7, 9)
                .await
                .unwrap()
                .data
                .into_bytes(),
            Bytes::from_static(b"file.txt:7:9")
        );
        assert_eq!(
            Handler::readdir(&mut session, 2, dir).await.unwrap().files[0].filename,
            "/tmp"
        );
        Handler::close(&mut session, 5, file.clone()).await.unwrap();

        assert_eq!((session.handler.reads, session.handler.readdirs), (1, 1));
        assert_eq!(session.handler.closed_files, vec!["file.txt"]);
        assert_eq!(
            Handler::read(&mut session, 6, file, 0, 1)
                .await
                .unwrap_err(),
            StatusCode::Failure
        );
    }

    #[tokio::test]
    async fn rejects_wrong_foreign_and_malformed_handles_before_user_handler() {
        let mut owner = ManagedSession::new(TestHandler::default());
        let mut other = ManagedSession::new(TestHandler::default());
        let (file, dir) = handles(&mut owner).await;

        assert_eq!(
            Handler::read(&mut owner, 2, dir, 0, 1).await.unwrap_err(),
            StatusCode::Failure
        );
        assert_eq!(
            Handler::readdir(&mut owner, 2, file.clone())
                .await
                .unwrap_err(),
            StatusCode::Failure
        );
        assert_eq!(
            Handler::read(&mut owner, 2, Bytes::from_static(b"bad"), 0, 1)
                .await
                .unwrap_err(),
            StatusCode::Failure
        );
        assert_eq!(
            Handler::read(&mut other, 2, file, 0, 1).await.unwrap_err(),
            StatusCode::Failure
        );
        assert_eq!((owner.handler.reads, owner.handler.readdirs), (0, 0));
        assert_eq!(other.handler.reads, 0);
    }
}
