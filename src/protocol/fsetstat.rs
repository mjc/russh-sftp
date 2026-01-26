use bytes::Bytes;

use super::{impl_packet_for, impl_request_id, FileAttributes, Packet, RequestId};

/// Implementation for `SSH_FXP_FSETSTAT`
#[derive(Debug, Serialize, Deserialize)]
pub struct FSetStat {
    pub id: u32,
    /// File handle (opaque bytes, use `handle_str()` for display).
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
    pub attrs: FileAttributes,
}

impl FSetStat {
    /// Get handle as string (lossy UTF-8 conversion for display/logging).
    pub fn handle_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.handle)
    }
}

impl_request_id!(FSetStat);
impl_packet_for!(FSetStat);
