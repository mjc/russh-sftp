use bytes::Buf;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};
use crate::{de::data_deserialize, ser::data_serialize};

// TODO: Change `data` field from Vec<u8> to Bytes to avoid copies.
// Requires updating RawSftpSession::extended(), Handler::extended(), and
// the extension structs' TryInto implementations (breaking change).

fn remaining_to_vec<B: Buf>(input: &mut B) -> Vec<u8> {
    let mut data = vec![0; input.remaining()];
    input.copy_to_slice(&mut data);
    data
}

/// Implementation for `SSH_FXP_EXTENDED`
#[derive(Debug, Serialize, Deserialize)]
pub struct Extended {
    pub id: u32,
    pub request: String,
    #[serde(serialize_with = "data_serialize")]
    #[serde(deserialize_with = "data_deserialize")]
    pub data: Vec<u8>,
}

impl Extended {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            request: input.try_get_string()?,
            data: remaining_to_vec(input),
        })
    }
}

impl_request_id!(Extended);
impl_packet_for!(Extended);

/// Implementation for `SSH_FXP_EXTENDED_REPLY`
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedReply {
    pub id: u32,
    #[serde(serialize_with = "data_serialize")]
    #[serde(deserialize_with = "data_deserialize")]
    pub data: Vec<u8>,
}

impl ExtendedReply {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            data: remaining_to_vec(input),
        })
    }
}

impl_request_id!(ExtendedReply);
impl_packet_for!(ExtendedReply);
