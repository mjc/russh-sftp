use bytes::{Buf, BufMut, Bytes, BytesMut};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_DATA`
#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    pub id: u32,
    #[serde(deserialize_with = "crate::de::bytes_deserialize")]
    #[serde(serialize_with = "crate::ser::bytes_serialize")]
    pub data: Bytes,
}

impl Data {
    pub fn new(id: u32, data: impl Into<Bytes>) -> Self {
        Self {
            id,
            data: data.into(),
        }
    }

    pub fn from_vec(id: u32, data: Vec<u8>) -> Self {
        Self::new(id, Bytes::from(data))
    }

    /// Zero-copy deserialization from Bytes.
    /// This bypasses serde to avoid the Vec allocation in the data field.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let data = input.try_get_bytes()?;
        Ok(Data { id, data })
    }

    pub(crate) fn serialize_into(&self, output: &mut BytesMut) -> Result<(), Error> {
        output.put_u32(self.id);
        output.put_u32(
            u32::try_from(self.data.len())
                .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?,
        );
        output.put_slice(&self.data);
        Ok(())
    }

    pub fn data_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.data.to_vec()
    }
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
