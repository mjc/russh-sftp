use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_READDIR`
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadDir {
    pub id: u32,
    /// Directory handle (opaque bytes, use `handle_str()` for display).
    /// Zero-copy from packet buffer.
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
}

impl ReadDir {
    pub fn new(id: u32, handle: impl Into<Bytes>) -> Self {
        Self {
            id,
            handle: handle.into(),
        }
    }

    pub fn from_string(id: u32, handle: impl Into<String>) -> Self {
        Self::new(id, Bytes::from(handle.into()))
    }

    /// Deserialize from Bytes with zero-copy handle.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let handle = input.try_get_bytes()?;
        Ok(ReadDir { id, handle })
    }

    /// Get handle as string (lossy UTF-8 conversion for display/logging).
    pub fn handle_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.handle)
    }

    pub fn handle_string(&self) -> String {
        self.handle_str().into_owned()
    }

    pub fn into_handle_string(self) -> String {
        String::from_utf8_lossy(&self.handle).into_owned()
    }
}

impl_request_id!(ReadDir);
impl_packet_for!(ReadDir);
