use bytes::Buf;

use super::{impl_packet_for, impl_request_id, FileAttributes, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_MKDIR`
#[derive(Debug, Serialize, Deserialize)]
pub struct MkDir {
    pub id: u32,
    pub path: String,
    pub attrs: FileAttributes,
}

impl MkDir {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            path: input.try_get_string()?,
            attrs: FileAttributes::from_bytes(input)?,
        })
    }
}

impl_request_id!(MkDir);
impl_packet_for!(MkDir);
