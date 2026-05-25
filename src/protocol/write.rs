use bytes::{Buf, Bytes};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_WRITE`
#[derive(Debug, Serialize, Deserialize)]
pub struct Write {
    pub id: u32,
    /// File handle (opaque bytes). Zero-copy from packet buffer.
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub handle: Bytes,
    pub offset: u64,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub data: Bytes,
}

impl Write {
    pub fn new(id: u32, handle: Bytes, offset: u64, data: Bytes) -> Self {
        Self {
            id,
            handle,
            offset,
            data,
        }
    }

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
}

impl_request_id!(Write);
impl_packet_for!(Write);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ser;
    use bytes::{BufMut, BytesMut};

    #[test]
    fn write_from_bytes_roundtrips_empty_small_and_large_payloads() {
        for (id, handle, offset, data) in [
            (1, Bytes::from_static(b"h"), 0, Bytes::new()),
            (
                42,
                Bytes::from_static(b"test-handle"),
                1024,
                Bytes::from_static(b"hello world"),
            ),
            (
                999,
                Bytes::from_static(b"big-file"),
                u64::MAX,
                Bytes::from(vec![0xAB; 32 * 1024]),
            ),
        ] {
            let original = Write {
                id,
                handle,
                offset,
                data,
            };

            let serialized = ser::to_bytes(&original).expect("serialize failed");
            let mut bytes = serialized;
            let deserialized = Write::from_bytes(&mut bytes).expect("deserialize failed");

            assert_eq!(deserialized.id, original.id);
            assert_eq!(deserialized.handle, original.handle);
            assert_eq!(deserialized.offset, original.offset);
            assert_eq!(deserialized.data, original.data);
        }
    }

    #[test]
    fn write_from_bytes_rejects_truncated_payload() {
        let mut bytes = BytesMut::new();
        bytes.put_u32(11);
        bytes.put_u32(6);
        bytes.extend_from_slice(b"handle");
        bytes.put_u64(99);
        bytes.put_u32(4);
        bytes.extend_from_slice(b"dat");

        assert!(Write::from_bytes(&mut bytes.freeze()).is_err());
    }
}
