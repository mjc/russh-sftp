use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Deserializer, Serializer};

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::buf::TryBuf;
use crate::error::Error;

/// Implementation for `SSH_FXP_DATA`
#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    pub id: u32,
    #[serde(deserialize_with = "data_payload_deserialize")]
    #[serde(serialize_with = "data_payload_serialize")]
    pub data: DataPayload,
}

impl Data {
    pub fn new(id: u32, data: Bytes) -> Self {
        Self {
            id,
            data: data.into(),
        }
    }

    /// Zero-copy deserialization from Bytes.
    /// This bypasses serde to avoid the Vec allocation in the data field.
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, Error> {
        let id = input.try_get_u32()?;
        let data = input.try_get_bytes()?;
        Ok(Data {
            id,
            data: data.into(),
        })
    }

    pub(crate) fn serialize_into(&self, output: &mut BytesMut) -> Result<(), Error> {
        output.put_u32(self.id);
        output.put_u32(
            u32::try_from(self.data.len())
                .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?,
        );
        output.put_slice(self.data.as_ref());
        Ok(())
    }
}

#[derive(Debug)]
pub enum DataPayload {
    Bytes(Bytes),
    #[cfg(feature = "russh-channel-data")]
    Channel(russh::ChannelData),
}

impl DataPayload {
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn into_bytes(self) -> Bytes {
        match self {
            Self::Bytes(data) => data,
            #[cfg(feature = "russh-channel-data")]
            Self::Channel(data) => Bytes::copy_from_slice(data.as_ref()),
        }
    }

    pub(crate) fn try_prepend(&mut self, prefix: &[u8]) -> bool {
        let _ = prefix;
        match self {
            Self::Bytes(_) => false,
            #[cfg(feature = "russh-channel-data")]
            Self::Channel(data) => data.try_prepend(prefix),
        }
    }

    #[cfg(feature = "russh-channel-data")]
    pub fn into_channel_data(self) -> russh::ChannelData {
        match self {
            Self::Bytes(data) => data.into(),
            Self::Channel(data) => data,
        }
    }
}

impl AsRef<[u8]> for DataPayload {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Bytes(data) => data.as_ref(),
            #[cfg(feature = "russh-channel-data")]
            Self::Channel(data) => data.as_ref(),
        }
    }
}

impl From<Bytes> for DataPayload {
    fn from(data: Bytes) -> Self {
        Self::Bytes(data)
    }
}

impl From<Vec<u8>> for DataPayload {
    fn from(data: Vec<u8>) -> Self {
        Self::Bytes(data.into())
    }
}

#[cfg(feature = "russh-channel-data")]
impl From<russh::ChannelData> for DataPayload {
    fn from(data: russh::ChannelData) -> Self {
        Self::Channel(data)
    }
}

impl PartialEq<Bytes> for DataPayload {
    fn eq(&self, other: &Bytes) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl PartialEq for DataPayload {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

fn data_payload_serialize<S>(data: &DataPayload, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(data.as_ref())
}

fn data_payload_deserialize<'de, D>(deserializer: D) -> Result<DataPayload, D::Error>
where
    D: Deserializer<'de>,
{
    crate::de::bytes_deserialize(deserializer).map(DataPayload::from)
}

impl_request_id!(Data);
impl_packet_for!(Data);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{de, ser};

    #[test]
    fn data_roundtrips_empty_small_and_large_payloads() {
        for (id, data) in [
            (1, Bytes::new()),
            (42, Bytes::from_static(b"file contents here")),
            (999, Bytes::from(vec![0xCD; 32 * 1024])),
        ] {
            let original = Data {
                id,
                data: data.into(),
            };

            let serialized = ser::to_bytes(&original).expect("serialize failed");
            let mut bytes = serialized;
            let deserialized: Data = de::from_bytes(&mut bytes).expect("deserialize failed");

            assert_eq!(deserialized.id, original.id);
            assert_eq!(deserialized.data, original.data);
        }
    }

    #[test]
    fn data_from_bytes_parses_directly() {
        let mut bytes = BytesMut::new();
        bytes.put_u32(7);
        bytes.put_u32(5);
        bytes.extend_from_slice(b"hello");

        let parsed = Data::from_bytes(&mut bytes.freeze()).expect("parse data");

        assert_eq!(parsed.id, 7);
        assert_eq!(parsed.data, Bytes::from_static(b"hello"));
    }
}
