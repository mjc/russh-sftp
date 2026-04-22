use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_READ`
#[derive(Debug, Serialize, Deserialize)]
pub struct Read {
    pub id: u32,
    /// File handle (opaque bytes, use `handle_str()` for display).
    /// Zero-copy from packet buffer.
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
    pub offset: u64,
    pub len: u32,
}

impl Read {
    /// Deserialize from Bytes with zero-copy handle.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let handle = input.try_get_bytes()?;
        let offset = input.try_get_u64()?;
        let len = input.try_get_u32()?;
        Ok(Read {
            id,
            handle,
            offset,
            len,
        })
    }

    /// Get handle as string (lossy UTF-8 conversion for display/logging).
    pub fn handle_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.handle)
    }
}

impl_request_id!(Read);
impl_packet_for!(Read);
