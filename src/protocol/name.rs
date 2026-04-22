use bytes::Buf;
use serde::{Deserialize, Serialize};

use super::{impl_packet_for, impl_request_id, File, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

const MAX_NAME_ENTRIES: usize = 16 * 1024;
const MAX_NAME_PREALLOC: usize = 256;

/// Implementation for `SSH_FXP_NAME`
#[derive(Debug, Serialize, Deserialize)]
pub struct Name {
    pub id: u32,
    pub files: Vec<File>,
}

impl Name {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let count = input.try_get_u32()? as usize;
        let max_count = input.remaining() / 12;
        if count > max_count {
            return Err(Error::BadMessage(format!(
                "name count {count} exceeds maximum possible entries {max_count}"
            )));
        }
        if count > MAX_NAME_ENTRIES {
            return Err(Error::BadMessage(format!(
                "name count {count} exceeds maximum supported entries {MAX_NAME_ENTRIES}"
            )));
        }
        let mut files = Vec::with_capacity(count.min(MAX_NAME_PREALLOC));
        for _ in 0..count {
            files.push(File::from_bytes(input)?);
        }
        Ok(Self { id, files })
    }
}

impl_request_id!(Name);
impl_packet_for!(Name);
