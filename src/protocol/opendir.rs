use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_OPENDIR`
#[derive(Debug, Serialize, Deserialize)]
pub struct OpenDir {
    pub id: u32,
    pub path: String,
}

impl OpenDir {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            path: input.try_get_string()?,
        })
    }
}

impl_request_id!(OpenDir);
impl_packet_for!(OpenDir);
