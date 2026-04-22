use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_HANDLE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Handle {
    pub id: u32,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
}

impl Handle {
    pub fn new(id: u32, handle: Bytes) -> Self {
        Self { id, handle }
    }

    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            handle: input.try_get_bytes()?,
        })
    }
}

impl_request_id!(Handle);
impl_packet_for!(Handle);
