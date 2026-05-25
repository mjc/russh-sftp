use std::{collections::HashMap, marker::PhantomData};

const HANDLE_MAGIC: &str = "russh-sftp";

pub trait HandleTag {
    const KIND: char;
}

#[derive(Debug)]
pub enum FileTag {}

#[derive(Debug)]
pub enum DirTag {}

impl HandleTag for FileTag {
    const KIND: char = 'f';
}

impl HandleTag for DirTag {
    const KIND: char = 'd';
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionHandle<K> {
    raw: String,
    _kind: PhantomData<K>,
}

pub type FileHandle = SessionHandle<FileTag>;
pub type DirHandle = SessionHandle<DirTag>;

impl<K> SessionHandle<K> {
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn into_string(self) -> String {
        self.raw
    }
}

#[derive(Debug)]
pub struct SessionHandles<F, D> {
    files: HashMap<String, F>,
    dirs: HashMap<String, D>,
    next: u64,
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
            next: 0,
        }
    }

    pub fn insert_file(&mut self, value: F) -> FileHandle {
        let handle = self.new_handle::<FileTag>();
        self.files.insert(handle.as_str().to_owned(), value);
        handle
    }

    pub fn insert_dir(&mut self, value: D) -> DirHandle {
        let handle = self.new_handle::<DirTag>();
        self.dirs.insert(handle.as_str().to_owned(), value);
        handle
    }

    pub fn get_file_mut(&mut self, handle: &FileHandle) -> Option<&mut F> {
        self.files.get_mut(handle.as_str())
    }

    pub fn get_dir_mut(&mut self, handle: &DirHandle) -> Option<&mut D> {
        self.dirs.get_mut(handle.as_str())
    }

    pub fn remove_file(&mut self, handle: &FileHandle) -> Option<F> {
        self.files.remove(handle.as_str())
    }

    pub fn remove_dir(&mut self, handle: &DirHandle) -> Option<D> {
        self.dirs.remove(handle.as_str())
    }

    pub fn decode_file(&self, raw: &str) -> Option<FileHandle> {
        let handle = self.decode::<FileTag>(raw)?;
        self.files.contains_key(handle.as_str()).then_some(handle)
    }

    pub fn decode_dir(&self, raw: &str) -> Option<DirHandle> {
        let handle = self.decode::<DirTag>(raw)?;
        self.dirs.contains_key(handle.as_str()).then_some(handle)
    }

    fn new_handle<K: HandleTag>(&mut self) -> SessionHandle<K> {
        let raw = format!("{HANDLE_MAGIC}:{}:{}", K::KIND, self.next);
        self.next += 1;
        SessionHandle {
            raw,
            _kind: PhantomData,
        }
    }

    fn decode<K: HandleTag>(&self, raw: &str) -> Option<SessionHandle<K>> {
        let mut parts = raw.split(':');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(HANDLE_MAGIC), Some(kind), Some(_), None)
                if kind.starts_with(K::KIND) && kind.len() == 1 =>
            {
                Some(SessionHandle {
                    raw: raw.to_owned(),
                    _kind: PhantomData,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_resolve_only_in_owner_and_kind() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let other = SessionHandles::<&str, &str>::new();
        let file = handles.insert_file("file");
        let dir = handles.insert_dir("dir");

        assert!(handles.decode_file(file.as_str()).is_some());
        assert!(handles.decode_dir(dir.as_str()).is_some());
        assert!(handles.decode_dir(file.as_str()).is_none());
        assert!(handles.decode_file(dir.as_str()).is_none());
        assert!(other.decode_file(file.as_str()).is_none());
    }

    #[test]
    fn rejects_stale_and_malformed_handles() {
        let mut handles = SessionHandles::<&str, &str>::new();
        let handle = handles.insert_file("file");
        let decoded = handles.decode_file(handle.as_str()).unwrap();

        assert_eq!(handles.remove_file(&decoded), Some("file"));
        assert!(handles.decode_file(handle.as_str()).is_none());
        assert!(handles.decode_file("").is_none());
        assert!(handles.decode_file("not-a-real-handle").is_none());
    }
}
