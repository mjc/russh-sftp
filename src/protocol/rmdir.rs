use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_RMDIR`
#[derive(Debug, Serialize, Deserialize)]
pub struct RmDir {
    pub id: u32,
    pub path: String,
}

impl RmDir {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            path: input.try_get_string()?,
        })
    }
}

impl_request_id!(RmDir);
impl_packet_for!(RmDir);
