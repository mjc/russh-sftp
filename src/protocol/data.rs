use bytes::Bytes;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};

/// Implementation for `SSH_FXP_DATA`
#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    pub id: u32,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub data: Bytes,
}

impl_request_id!(Data);
impl_packet_for!(Data);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{de, ser};

    #[test]
    fn data_roundtrip() {
        let original = Data {
            id: 42,
            data: Bytes::from_static(b"file contents here"),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: Data = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.data, original.data);
    }

    #[test]
    fn data_empty() {
        let original = Data {
            id: 1,
            data: Bytes::new(),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: Data = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.data.len(), 0);
    }

    #[test]
    fn data_large() {
        let large_data = vec![0xCDu8; 32 * 1024]; // 32KB
        let original = Data {
            id: 999,
            data: Bytes::from(large_data.clone()),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: Data = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.data.as_ref(), large_data.as_slice());
    }
}
