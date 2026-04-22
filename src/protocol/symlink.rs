use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_SYMLINK`
#[derive(Debug, Serialize, Deserialize)]
pub struct Symlink {
    pub id: u32,
    pub linkpath: String,
    pub targetpath: String,
}

impl Symlink {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            linkpath: input.try_get_string()?,
            targetpath: input.try_get_string()?,
        })
    }
}

impl_request_id!(Symlink);
impl_packet_for!(Symlink);
