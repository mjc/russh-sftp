use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error};

/// Implementation for `SSH_FXP_HANDLE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Handle {
    pub id: u32,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
}

impl Handle {
    pub fn new(id: u32, handle: impl Into<Bytes>) -> Self {
        Self {
            id,
            handle: handle.into(),
        }
    }

    pub fn from_string(id: u32, handle: impl Into<String>) -> Self {
        Self::new(id, Bytes::from(handle.into()))
    }

    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        Ok(Self {
            id: input.try_get_u32()?,
            handle: input.try_get_bytes()?,
        })
    }

    pub fn handle_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.handle)
    }

    pub fn handle_string(&self) -> String {
        self.handle_str().into_owned()
    }

    pub fn into_handle_string(self) -> String {
        String::from_utf8_lossy(&self.handle).into_owned()
    }
}

impl_request_id!(Handle);
impl_packet_for!(Handle);
