use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_REMOVE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Remove {
    pub id: u32,
    pub filename: String,
}

impl Remove {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            filename: input.try_get_string()?,
        })
    }
}

impl_request_id!(Remove);
impl_packet_for!(Remove);
