use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_CLOSE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Close {
    pub id: u32,
    /// File handle (opaque bytes). Zero-copy from packet buffer.
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
}

impl Close {
    pub fn new(id: u32, handle: Bytes) -> Self {
        Self { id, handle }
    }

    /// Deserialize from Bytes with zero-copy handle.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let handle = input.try_get_bytes()?;
        Ok(Close { id, handle })
    }
}

impl_request_id!(Close);
impl_packet_for!(Close);
