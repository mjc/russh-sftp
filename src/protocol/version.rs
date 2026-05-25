use std::collections::HashMap;

use bytes::Buf;

use super::{impl_packet_for, Packet, VERSION};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_VERSION`
#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub version: u32,
    pub extensions: HashMap<String, String>,
}

impl_packet_for!(Version);

impl Version {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let version = input.try_get_u32()?;
        let mut extensions = HashMap::new();
        while input.has_remaining() {
            let key = input.try_get_string()?;
            let value = input.try_get_string()?;
            extensions.insert(key, value);
        }

        Ok(Self {
            version,
            extensions,
        })
    }

    pub fn new() -> Self {
        Self {
            version: VERSION,
            extensions: HashMap::new(),
        }
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}
