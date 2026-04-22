use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_WRITE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Write {
    pub id: u32,
    /// File handle (opaque bytes, use `handle_str()` for display).
    /// Zero-copy from packet buffer.
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
    pub offset: u64,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub data: Bytes,
}

impl Write {
    /// Zero-copy deserialization from Bytes.
    /// This bypasses serde to avoid allocations.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let handle = input.try_get_bytes()?;
        let offset = input.try_get_u64()?;
        let data = input.try_get_bytes()?;
        Ok(Write {
            id,
            handle,
            offset,
            data,
        })
    }

    /// Get handle as string (lossy UTF-8 conversion for display/logging).
    pub fn handle_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.handle)
    }
}

impl_request_id!(Write);
impl_packet_for!(Write);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{de, ser};

    #[test]
    fn write_roundtrip() {
        let original = Write {
            id: 42,
            handle: Bytes::from_static(b"test-handle"),
            offset: 1024,
            data: Bytes::from_static(b"hello world"),
        };

        // Serialize
        let serialized = ser::to_bytes(&original).expect("serialize failed");

        // Deserialize
        let mut bytes = serialized;
        let deserialized: Write = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.handle, original.handle);
        assert_eq!(deserialized.offset, original.offset);
        assert_eq!(deserialized.data, original.data);
    }

    #[test]
    fn write_empty_data() {
        let original = Write {
            id: 1,
            handle: Bytes::from_static(b"h"),
            offset: 0,
            data: Bytes::new(),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: Write = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.data.len(), 0);
    }

    #[test]
    fn write_large_data() {
        let large_data = vec![0xABu8; 32 * 1024]; // 32KB
        let original = Write {
            id: 999,
            handle: Bytes::from_static(b"big-file"),
            offset: u64::MAX,
            data: Bytes::from(large_data.clone()),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: Write = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.data.as_ref(), large_data.as_slice());
    }
}
