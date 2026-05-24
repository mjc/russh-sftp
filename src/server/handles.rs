use bytes::Bytes;
use std::collections::HashMap;
use std::marker::PhantomData;

const HANDLE_MAGIC: u8 = b'H';
const HANDLE_LEN: usize = 18;

pub trait HandleTag {
    const KIND: u8;
}

#[derive(Debug)]
pub enum FileTag {}

#[derive(Debug)]
pub enum DirTag {}

impl HandleTag for FileTag {
    const KIND: u8 = 1;
}

impl HandleTag for DirTag {
    const KIND: u8 = 2;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionHandle<K> {
    bytes: Bytes,
    _kind: PhantomData<K>,
}

pub type FileHandle = SessionHandle<FileTag>;
pub type DirHandle = SessionHandle<DirTag>;

impl<K> SessionHandle<K> {
    pub fn as_bytes(&self) -> &Bytes {
        &self.bytes
    }

    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }
}

#[derive(Debug)]
pub struct SessionHandles<F, D> {
    files: HashMap<Bytes, F>,
    dirs: HashMap<Bytes, D>,
}

impl<F, D> Default for SessionHandles<F, D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, D> SessionHandles<F, D> {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            dirs: HashMap::new(),
        }
    }

    pub fn insert_file(&mut self, value: F) -> FileHandle {
        let handle = self.new_handle::<FileTag>();
        self.files.insert(handle.as_bytes().clone(), value);
        handle
    }

    pub fn insert_dir(&mut self, value: D) -> DirHandle {
        let handle = self.new_handle::<DirTag>();
        self.dirs.insert(handle.as_bytes().clone(), value);
        handle
    }

    /// Returns a file entry for a typed file handle.
    ///
    /// ```compile_fail
    /// # use russh_sftp::server::SessionHandles;
    /// let mut handles = SessionHandles::<&str, &str>::new();
    /// let dir = handles.insert_dir("dir");
    /// let _ = handles.get_file(&dir);
    /// ```
    pub fn get_file(&self, handle: &FileHandle) -> Option<&F> {
        self.files.get(handle.as_bytes())
    }

    pub fn get_file_mut(&mut self, handle: &FileHandle) -> Option<&mut F> {
        self.files.get_mut(handle.as_bytes())
    }

    /// Returns a directory entry for a typed directory handle.
    ///
    /// ```compile_fail
    /// # use russh_sftp::server::SessionHandles;
    /// let mut handles = SessionHandles::<&str, &str>::new();
    /// let file = handles.insert_file("file");
    /// let _ = handles.get_dir(&file);
    /// ```
    pub fn get_dir(&self, handle: &DirHandle) -> Option<&D> {
        self.dirs.get(handle.as_bytes())
    }

    pub fn get_dir_mut(&mut self, handle: &DirHandle) -> Option<&mut D> {
        self.dirs.get_mut(handle.as_bytes())
    }

    pub fn remove_file(&mut self, handle: &FileHandle) -> Option<F> {
        self.files.remove(handle.as_bytes())
    }

    pub fn remove_dir(&mut self, handle: &DirHandle) -> Option<D> {
        self.dirs.remove(handle.as_bytes())
    }

    pub fn decode_file(&self, bytes: &Bytes) -> Option<FileHandle> {
        let handle = self.decode::<FileTag>(bytes)?;
        self.files.contains_key(handle.as_bytes()).then_some(handle)
    }

    pub fn decode_dir(&self, bytes: &Bytes) -> Option<DirHandle> {
        let handle = self.decode::<DirTag>(bytes)?;
        self.dirs.contains_key(handle.as_bytes()).then_some(handle)
    }

    fn new_handle<K: HandleTag>(&self) -> SessionHandle<K> {
        let bytes = Self::new_opaque_bytes(K::KIND);
        SessionHandle {
            bytes,
            _kind: PhantomData,
        }
    }

    fn new_opaque_bytes(kind: u8) -> Bytes {
        let mut encoded = [0; HANDLE_LEN];
        encoded[0] = HANDLE_MAGIC;
        encoded[1] = kind;
        encoded[2..18].copy_from_slice(&rand::random::<[u8; 16]>());
        Bytes::copy_from_slice(&encoded)
    }

    fn decode<K: HandleTag>(&self, bytes: &Bytes) -> Option<SessionHandle<K>> {
        let raw: [u8; HANDLE_LEN] = bytes.as_ref().try_into().ok()?;
        if raw[0] != HANDLE_MAGIC || raw[1] != K::KIND {
            return None;
        }

        Some(SessionHandle {
            bytes: bytes.clone(),
            _kind: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_handles_are_unique_within_session() {
        let mut handles = SessionHandles::<&str, &str>::new();

        let first = handles.insert_file("first");
        let second = handles.insert_file("second");

        assert_ne!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn dir_handles_are_unique_within_session() {
        let mut handles = SessionHandles::<&str, &str>::new();

        let first = handles.insert_dir("first");
        let second = handles.insert_dir("second");

        assert_ne!(first.as_bytes(), second.as_bytes());
    }

    #[test]
    fn resolves_file_handle_in_owning_session() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_file("file");
        let decoded = handles.decode_file(handle.as_bytes()).expect("decode file");

        assert_eq!(handles.get_file(&decoded), Some(&"file"));
    }

    #[test]
    fn resolves_dir_handle_in_owning_session() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_dir("dir");
        let decoded = handles.decode_dir(handle.as_bytes()).expect("decode dir");

        assert_eq!(handles.get_dir(&decoded), Some(&"dir"));
    }

    #[test]
    fn rejects_file_handle_from_another_session() {
        let mut owner = SessionHandles::<&str, &str>::new();
        let other = SessionHandles::<&str, &str>::new();
        let handle = owner.insert_file("file");

        assert!(owner.decode_file(handle.as_bytes()).is_some());
        assert!(other.decode_file(handle.as_bytes()).is_none());
    }

    #[test]
    fn rejects_dir_handle_from_another_session() {
        let mut owner = SessionHandles::<&str, &str>::new();
        let other = SessionHandles::<&str, &str>::new();
        let handle = owner.insert_dir("dir");

        assert!(owner.decode_dir(handle.as_bytes()).is_some());
        assert!(other.decode_dir(handle.as_bytes()).is_none());
    }

    #[test]
    fn rejects_file_bytes_as_dir_handle() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_file("file");

        assert!(handles.decode_file(handle.as_bytes()).is_some());
        assert!(handles.decode_dir(handle.as_bytes()).is_none());
    }

    #[test]
    fn rejects_dir_bytes_as_file_handle() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_dir("dir");

        assert!(handles.decode_dir(handle.as_bytes()).is_some());
        assert!(handles.decode_file(handle.as_bytes()).is_none());
    }

    #[test]
    fn rejects_tampered_handle() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_file("file");
        let mut tampered = handle.as_bytes().to_vec();
        tampered[HANDLE_LEN - 1] ^= 0x01;

        assert!(handles.decode_file(handle.as_bytes()).is_some());
        assert!(handles.decode_file(&Bytes::from(tampered)).is_none());
    }

    #[test]
    fn rejects_stale_handle_after_remove() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_file("file");
        let decoded = handles.decode_file(handle.as_bytes()).expect("decode file");

        assert_eq!(handles.remove_file(&decoded), Some("file"));
        assert_eq!(handles.get_file(&decoded), None);
    }

    #[test]
    fn rejects_malformed_handles() {
        let handles = SessionHandles::<&str, &str>::new();

        assert!(handles.decode_file(&Bytes::new()).is_none());
        assert!(handles
            .decode_file(&Bytes::from_static(b"not-a-real-handle"))
            .is_none());
    }
}
