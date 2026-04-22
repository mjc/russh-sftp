use bytes::Buf;

use super::{impl_packet_for, impl_request_id, FileAttributes, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_ATTRS`
#[derive(Debug, Serialize, Deserialize)]
pub struct Attrs {
    pub id: u32,
    pub attrs: FileAttributes,
}

impl Attrs {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            attrs: FileAttributes::from_bytes(input)?,
        })
    }
}

impl_request_id!(Attrs);
impl_packet_for!(Attrs);
