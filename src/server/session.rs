use std::{collections::HashMap, future::Future};

use bytes::Bytes;

use super::{Handler, SessionHandles};
use crate::protocol::{
    Attrs, Data, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode, Version,
};

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
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn opendir(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Self::Dir, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
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
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn write<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
        _offset: u64,
        _data: Bytes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn readdir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn fstat_file<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn fstat_dir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn fsetstat_file<'a>(
        &'a mut self,
        _id: u32,
        _file: &'a mut Self::File,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn fsetstat_dir<'a>(
        &'a mut self,
        _id: u32,
        _dir: &'a mut Self::Dir,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn lstat(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn setstat(
        &mut self,
        _id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn remove(
        &mut self,
        _id: u32,
        _filename: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn mkdir(
        &mut self,
        _id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn rmdir(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn realpath(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn stat(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn rename(
        &mut self,
        _id: u32,
        _oldpath: String,
        _newpath: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn readlink(
        &mut self,
        _id: u32,
        _path: String,
    ) -> impl Future<Output = Result<Name, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn symlink(
        &mut self,
        _id: u32,
        _linkpath: String,
        _targetpath: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
    }

    fn extended(
        &mut self,
        _id: u32,
        _request: String,
        _data: Vec<u8>,
    ) -> impl Future<Output = Result<Packet, Self::Error>> + Send {
        let err = self.unimplemented();
        async move { Err(err) }
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
        let handle = self.handles.insert_file(file).into_bytes();
        Ok(Handle { id, handle })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let dir = self.handler.opendir(id, path).await.map_err(Into::into)?;
        let handle = self.handles.insert_dir(dir).into_bytes();
        Ok(Handle { id, handle })
    }

    async fn close(&mut self, id: u32, handle: Bytes) -> Result<Status, Self::Error> {
        if let Some(file_handle) = self.handles.decode_file(&handle) {
            let file = self
                .handles
                .remove_file(&file_handle)
                .ok_or(StatusCode::Failure)?;
            return self.handler.close_file(id, file).await.map_err(Into::into);
        }

        if let Some(dir_handle) = self.handles.decode_dir(&handle) {
            let dir = self
                .handles
                .remove_dir(&dir_handle)
                .ok_or(StatusCode::Failure)?;
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
        let handle = self
            .handles
            .decode_file(&handle)
            .ok_or(StatusCode::Failure)?;
        let file = self
            .handles
            .get_file_mut(&handle)
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
        let handle = self
            .handles
            .decode_file(&handle)
            .ok_or(StatusCode::Failure)?;
        let file = self
            .handles
            .get_file_mut(&handle)
            .ok_or(StatusCode::Failure)?;
        self.handler
            .write(id, file, offset, data)
            .await
            .map_err(Into::into)
    }

    async fn readdir(&mut self, id: u32, handle: Bytes) -> Result<Name, Self::Error> {
        let handle = self
            .handles
            .decode_dir(&handle)
            .ok_or(StatusCode::Failure)?;
        let dir = self
            .handles
            .get_dir_mut(&handle)
            .ok_or(StatusCode::Failure)?;
        self.handler.readdir(id, dir).await.map_err(Into::into)
    }

    async fn fstat(&mut self, id: u32, handle: Bytes) -> Result<Attrs, Self::Error> {
        if let Some(handle) = self.handles.decode_file(&handle) {
            let file = self
                .handles
                .get_file_mut(&handle)
                .ok_or(StatusCode::Failure)?;
            return self.handler.fstat_file(id, file).await.map_err(Into::into);
        }

        if let Some(handle) = self.handles.decode_dir(&handle) {
            let dir = self
                .handles
                .get_dir_mut(&handle)
                .ok_or(StatusCode::Failure)?;
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
        if let Some(handle) = self.handles.decode_file(&handle) {
            let file = self
                .handles
                .get_file_mut(&handle)
                .ok_or(StatusCode::Failure)?;
            return self
                .handler
                .fsetstat_file(id, file, attrs)
                .await
                .map_err(Into::into);
        }

        if let Some(handle) = self.handles.decode_dir(&handle) {
            let dir = self
                .handles
                .get_dir_mut(&handle)
                .ok_or(StatusCode::Failure)?;
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
        fstat_files: usize,
        fstat_dirs: usize,
        fsetstat_files: Vec<Option<u32>>,
        fsetstat_dirs: Vec<Option<u32>>,
        closed_files: Vec<String>,
        closed_dirs: Vec<String>,
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

        fn close_dir(
            &mut self,
            id: u32,
            dir: Self::Dir,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
            self.closed_dirs.push(dir);
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

        fn fstat_file<'a>(
            &'a mut self,
            id: u32,
            file: &'a mut Self::File,
        ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
            self.fstat_files += 1;
            async move {
                Ok(Attrs {
                    id,
                    attrs: attrs_with_size(file.len() as u64),
                })
            }
        }

        fn fstat_dir<'a>(
            &'a mut self,
            id: u32,
            dir: &'a mut Self::Dir,
        ) -> impl Future<Output = Result<Attrs, Self::Error>> + Send + 'a {
            self.fstat_dirs += 1;
            async move {
                Ok(Attrs {
                    id,
                    attrs: attrs_with_size(dir.len() as u64),
                })
            }
        }

        fn fsetstat_file<'a>(
            &'a mut self,
            id: u32,
            _file: &'a mut Self::File,
            attrs: FileAttributes,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
            self.fsetstat_files.push(attrs.permissions);
            async move { Ok(ok_status(id)) }
        }

        fn fsetstat_dir<'a>(
            &'a mut self,
            id: u32,
            _dir: &'a mut Self::Dir,
            attrs: FileAttributes,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send + 'a {
            self.fsetstat_dirs.push(attrs.permissions);
            async move { Ok(ok_status(id)) }
        }
    }

    fn attrs_with_size(size: u64) -> FileAttributes {
        FileAttributes {
            size: Some(size),
            ..FileAttributes::empty()
        }
    }

    async fn open_file(session: &mut ManagedSession<TestHandler>) -> Bytes {
        Handler::open(
            session,
            1,
            "file.txt".to_string(),
            OpenFlags::READ,
            FileAttributes::empty(),
        )
        .await
        .expect("open")
        .handle
    }

    async fn open_dir(session: &mut ManagedSession<TestHandler>) -> Bytes {
        Handler::opendir(session, 1, "/tmp".to_string())
            .await
            .expect("opendir")
            .handle
    }

    #[tokio::test]
    async fn read_decodes_wire_file_handle_to_typed_file_state() {
        let mut session = ManagedSession::new(TestHandler::default());
        let handle = open_file(&mut session).await;

        let data = Handler::read(&mut session, 2, handle, 7, 9)
            .await
            .expect("read");

        assert_eq!(data.data.as_ref(), b"file.txt:7:9");
        assert_eq!(session.handler.reads, 1);
    }

    #[tokio::test]
    async fn readdir_decodes_wire_dir_handle_to_typed_dir_state() {
        let mut session = ManagedSession::new(TestHandler::default());
        let handle = open_dir(&mut session).await;

        let names = Handler::readdir(&mut session, 2, handle)
            .await
            .expect("readdir");

        assert_eq!(names.files[0].filename, "/tmp");
        assert_eq!(session.handler.readdirs, 1);
    }

    #[tokio::test]
    async fn read_rejects_dir_handle_before_user_handler() {
        let mut session = ManagedSession::new(TestHandler::default());
        let handle = open_dir(&mut session).await;

        let result = Handler::read(&mut session, 2, handle, 0, 1).await;

        assert_eq!(result.unwrap_err(), StatusCode::Failure);
        assert_eq!(session.handler.reads, 0);
    }

    #[tokio::test]
    async fn readdir_rejects_file_handle_before_user_handler() {
        let mut session = ManagedSession::new(TestHandler::default());
        let handle = open_file(&mut session).await;

        let result = Handler::readdir(&mut session, 2, handle).await;

        assert_eq!(result.unwrap_err(), StatusCode::Failure);
        assert_eq!(session.handler.readdirs, 0);
    }

    #[tokio::test]
    async fn rejects_handle_from_another_managed_session() {
        let mut owner = ManagedSession::new(TestHandler::default());
        let mut other = ManagedSession::new(TestHandler::default());
        let handle = open_file(&mut owner).await;

        let result = Handler::read(&mut other, 2, handle, 0, 1).await;

        assert_eq!(result.unwrap_err(), StatusCode::Failure);
        assert_eq!(other.handler.reads, 0);
    }

    #[tokio::test]
    async fn rejects_tampered_handle_before_user_handler() {
        let mut session = ManagedSession::new(TestHandler::default());
        let mut tampered = open_file(&mut session).await.to_vec();
        tampered[3] ^= 0x80;

        let result = Handler::read(&mut session, 2, Bytes::from(tampered), 0, 1).await;

        assert_eq!(result.unwrap_err(), StatusCode::Failure);
        assert_eq!(session.handler.reads, 0);
    }

    #[tokio::test]
    async fn close_removes_file_handle_and_returns_owned_state_to_handler() {
        let mut session = ManagedSession::new(TestHandler::default());
        let raw = open_file(&mut session).await;

        Handler::close(&mut session, 2, raw.clone())
            .await
            .expect("close");
        let result = Handler::read(&mut session, 3, raw, 0, 1).await;

        assert_eq!(session.handler.closed_files, vec!["file.txt"]);
        assert_eq!(result.unwrap_err(), StatusCode::Failure);
    }

    #[tokio::test]
    async fn fstat_dispatches_file_and_dir_handles_to_typed_methods() {
        let mut session = ManagedSession::new(TestHandler::default());
        let file_handle = open_file(&mut session).await;
        let dir_handle = open_dir(&mut session).await;

        let file_attrs = Handler::fstat(&mut session, 3, file_handle)
            .await
            .expect("file fstat");
        let dir_attrs = Handler::fstat(&mut session, 4, dir_handle)
            .await
            .expect("dir fstat");

        assert_eq!(file_attrs.attrs.size, Some(8));
        assert_eq!(dir_attrs.attrs.size, Some(4));
        assert_eq!(session.handler.fstat_files, 1);
        assert_eq!(session.handler.fstat_dirs, 1);
    }

    #[tokio::test]
    async fn fsetstat_dispatches_file_and_dir_handles_to_typed_methods() {
        let mut session = ManagedSession::new(TestHandler::default());
        let file_handle = open_file(&mut session).await;
        let dir_handle = open_dir(&mut session).await;

        Handler::fsetstat(
            &mut session,
            3,
            file_handle,
            FileAttributes {
                permissions: Some(0o644),
                ..FileAttributes::empty()
            },
        )
        .await
        .expect("file fsetstat");
        Handler::fsetstat(
            &mut session,
            4,
            dir_handle,
            FileAttributes {
                permissions: Some(0o755),
                ..FileAttributes::empty()
            },
        )
        .await
        .expect("dir fsetstat");

        assert_eq!(session.handler.fsetstat_files, vec![Some(0o644)]);
        assert_eq!(session.handler.fsetstat_dirs, vec![Some(0o755)]);
    }

    #[tokio::test]
    async fn fstat_rejects_handle_from_another_session_before_user_handler() {
        let mut owner = ManagedSession::new(TestHandler::default());
        let mut other = ManagedSession::new(TestHandler::default());
        let handle = open_file(&mut owner).await;

        let result = Handler::fstat(&mut other, 2, handle).await;

        assert_eq!(result.unwrap_err(), StatusCode::Failure);
        assert_eq!(other.handler.fstat_files, 0);
        assert_eq!(other.handler.fstat_dirs, 0);
    }
}
