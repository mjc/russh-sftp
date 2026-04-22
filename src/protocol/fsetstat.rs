use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, FileAttributes, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_FSETSTAT`
#[derive(Debug, Serialize, Deserialize)]
pub struct FSetStat {
    pub id: u32,
    /// File handle (opaque bytes).
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
    pub attrs: FileAttributes,
}

impl FSetStat {
    pub fn new(id: u32, handle: Bytes, attrs: FileAttributes) -> Self {
        Self { id, handle, attrs }
    }

    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            handle: input.try_get_bytes()?,
            attrs: FileAttributes::from_bytes(input)?,
        })
    }
}

impl_request_id!(FSetStat);
impl_packet_for!(FSetStat);
