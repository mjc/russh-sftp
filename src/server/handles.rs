use std::{
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

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
    index: usize,
    generation: u64,
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
    files: Vec<Slot<F>>,
    dirs: Vec<Slot<D>>,
    free_file: Option<usize>,
    free_dir: Option<usize>,
    session: u64,
}

#[derive(Debug)]
struct Slot<T> {
    generation: u64,
    state: SlotState<T>,
}

#[derive(Debug)]
enum SlotState<T> {
    Occupied(T),
    Vacant { next: Option<usize> },
}

impl<F, D> Default for SessionHandles<F, D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F, D> SessionHandles<F, D> {
    pub fn new() -> Self {
        let session = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            files: Vec::new(),
            dirs: Vec::new(),
            free_file: None,
            free_dir: None,
            session,
        }
    }

    pub fn insert_file(&mut self, value: F) -> FileHandle {
        let (index, generation) = Self::insert_slot(&mut self.files, &mut self.free_file, value);
        let handle = self.new_handle::<FileTag>(index, generation);
        handle
    }

    pub fn insert_dir(&mut self, value: D) -> DirHandle {
        let (index, generation) = Self::insert_slot(&mut self.dirs, &mut self.free_dir, value);
        let handle = self.new_handle::<DirTag>(index, generation);
        handle
    }

    pub fn get_file_mut(&mut self, handle: &FileHandle) -> Option<&mut F> {
        Self::get_slot_mut(&mut self.files, handle.index, handle.generation)
    }

    pub fn get_dir_mut(&mut self, handle: &DirHandle) -> Option<&mut D> {
        Self::get_slot_mut(&mut self.dirs, handle.index, handle.generation)
    }

    pub(crate) fn get_file_mut_raw(&mut self, raw: &str) -> Option<&mut F> {
        let (index, generation) = self.decode_index::<FileTag>(raw)?;
        Self::get_slot_mut(&mut self.files, index, generation)
    }

    pub(crate) fn get_dir_mut_raw(&mut self, raw: &str) -> Option<&mut D> {
        let (index, generation) = self.decode_index::<DirTag>(raw)?;
        Self::get_slot_mut(&mut self.dirs, index, generation)
    }

    pub fn remove_file(&mut self, handle: &FileHandle) -> Option<F> {
        Self::remove_slot(
            &mut self.files,
            &mut self.free_file,
            handle.index,
            handle.generation,
        )
    }

    pub fn remove_dir(&mut self, handle: &DirHandle) -> Option<D> {
        Self::remove_slot(
            &mut self.dirs,
            &mut self.free_dir,
            handle.index,
            handle.generation,
        )
    }

    pub(crate) fn remove_file_raw(&mut self, raw: &str) -> Option<F> {
        let (index, generation) = self.decode_index::<FileTag>(raw)?;
        Self::remove_slot(&mut self.files, &mut self.free_file, index, generation)
    }

    pub(crate) fn remove_dir_raw(&mut self, raw: &str) -> Option<D> {
        let (index, generation) = self.decode_index::<DirTag>(raw)?;
        Self::remove_slot(&mut self.dirs, &mut self.free_dir, index, generation)
    }

    pub fn decode_file(&self, raw: &str) -> Option<FileHandle> {
        let (index, generation) = self.decode_index::<FileTag>(raw)?;
        let handle = self.new_decoded_handle(raw, index, generation);
        Self::get_slot(&self.files, index, generation)?;
        Some(handle)
    }

    pub fn decode_dir(&self, raw: &str) -> Option<DirHandle> {
        let (index, generation) = self.decode_index::<DirTag>(raw)?;
        let handle = self.new_decoded_handle(raw, index, generation);
        Self::get_slot(&self.dirs, index, generation)?;
        Some(handle)
    }

    fn insert_slot<T>(
        slots: &mut Vec<Slot<T>>,
        free: &mut Option<usize>,
        value: T,
    ) -> (usize, u64) {
        if let Some(index) = *free {
            let slot = &mut slots[index];
            let SlotState::Vacant { next } = slot.state else {
                unreachable!("free list points at occupied slot");
            };
            *free = next;
            slot.state = SlotState::Occupied(value);
            (index, slot.generation)
        } else {
            let index = slots.len();
            slots.push(Slot {
                generation: 0,
                state: SlotState::Occupied(value),
            });
            (index, 0)
        }
    }

    fn remove_slot<T>(
        slots: &mut [Slot<T>],
        free: &mut Option<usize>,
        index: usize,
        generation: u64,
    ) -> Option<T> {
        let slot = slots.get_mut(index)?;
        if slot.generation != generation {
            return None;
        }

        let state = std::mem::replace(&mut slot.state, SlotState::Vacant { next: *free });
        let SlotState::Occupied(value) = state else {
            return None;
        };

        slot.generation = slot.generation.wrapping_add(1);
        *free = Some(index);
        Some(value)
    }

    fn get_slot<T>(slots: &[Slot<T>], index: usize, generation: u64) -> Option<&T> {
        let slot = slots.get(index)?;
        if slot.generation != generation {
            return None;
        }

        match &slot.state {
            SlotState::Occupied(value) => Some(value),
            SlotState::Vacant { .. } => None,
        }
    }

    fn get_slot_mut<T>(slots: &mut [Slot<T>], index: usize, generation: u64) -> Option<&mut T> {
        let slot = slots.get_mut(index)?;
        if slot.generation != generation {
            return None;
        }

        match &mut slot.state {
            SlotState::Occupied(value) => Some(value),
            SlotState::Vacant { .. } => None,
        }
    }

    fn new_handle<K: HandleTag>(&self, index: usize, generation: u64) -> SessionHandle<K> {
        let mut raw = String::with_capacity(
            3 + decimal_len_u64(self.session)
                + decimal_len_usize(index)
                + decimal_len_u64(generation),
        );
        raw.push(K::KIND);
        push_u64(&mut raw, self.session);
        raw.push(':');
        push_usize(&mut raw, index);
        raw.push(':');
        push_u64(&mut raw, generation);

        SessionHandle {
            raw,
            index,
            generation,
            _kind: PhantomData,
        }
    }

    fn new_decoded_handle<K>(&self, raw: &str, index: usize, generation: u64) -> SessionHandle<K> {
        SessionHandle {
            raw: raw.to_owned(),
            index,
            generation,
            _kind: PhantomData,
        }
    }

    fn decode_index<K: HandleTag>(&self, raw: &str) -> Option<(usize, u64)> {
        if raw.as_bytes().first().copied()? != K::KIND as u8 {
            return None;
        }

        let mut fields = raw[1..].splitn(3, ':');
        let session = parse_u64(fields.next()?)?;
        if session != self.session {
            return None;
        }

        Some((parse_usize(fields.next()?)?, parse_u64(fields.next()?)?))
    }
}

fn decimal_len_usize(mut value: usize) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

fn decimal_len_u64(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

fn push_usize(output: &mut String, mut value: usize) {
    let mut digits = [0; 20];
    let mut len = 0;

    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }

    output.reserve(len);
    for digit in digits[..len].iter().rev() {
        output.push(*digit as char);
    }
}

fn push_u64(output: &mut String, mut value: u64) {
    let mut digits = [0; 20];
    let mut len = 0;

    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }

    output.reserve(len);
    for digit in digits[..len].iter().rev() {
        output.push(*digit as char);
    }
}

fn parse_usize(raw: &str) -> Option<usize> {
    let mut value = 0usize;
    let mut digits = 0;

    for byte in raw.bytes() {
        let digit = byte.checked_sub(b'0')?;
        if digit > 9 {
            return None;
        }

        value = value.checked_mul(10)?.checked_add(digit as usize)?;
        digits += 1;
    }

    (digits > 0).then_some(value)
}

fn parse_u64(raw: &str) -> Option<u64> {
    let mut value = 0u64;
    let mut digits = 0;

    for byte in raw.bytes() {
        let digit = byte.checked_sub(b'0')?;
        if digit > 9 {
            return None;
        }

        value = value.checked_mul(10)?.checked_add(digit as u64)?;
        digits += 1;
    }

    (digits > 0).then_some(value)
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
        let stale = handle.as_str().to_owned();

        assert_eq!(handles.remove_file(&decoded), Some("file"));
        let replacement = handles.insert_file("replacement");

        assert!(handles.decode_file(&stale).is_none());
        assert_eq!(
            handles.get_file_mut(&handles.decode_file(replacement.as_str()).unwrap()),
            Some(&mut "replacement")
        );
        assert!(handles.decode_file("").is_none());
        assert!(handles.decode_file("not-a-real-handle").is_none());
    }
}
