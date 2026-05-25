use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_RENAME`
#[derive(Debug, Serialize, Deserialize)]
pub struct Rename {
    pub id: u32,
    pub oldpath: String,
    pub newpath: String,
}

impl Rename {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            oldpath: input.try_get_string()?,
            newpath: input.try_get_string()?,
        })
    }
}

impl_request_id!(Rename);
impl_packet_for!(Rename);
