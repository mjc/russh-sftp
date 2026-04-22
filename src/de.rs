use bytes::{Buf, BufMut, Bytes};
use serde::de::{EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor};
use std::fmt;

use crate::{buf::TryBuf, error::Error};

pub struct Deserializer<'a> {
    input: &'a mut Bytes,
}

/// Converting bytes to protocol-compliant type
pub fn from_bytes<'a, T>(bytes: &'a mut Bytes) -> Result<T, Error>
where
    T: serde::Deserialize<'a>,
{
    let mut deserializer = Deserializer { input: bytes };
    T::deserialize(&mut deserializer)
}

/// Deserialization of a [`Vec`] without length. Usually reads until the end byte
/// or end of the packet because the size is unknown.
pub fn data_deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct DataVisitor;

    impl<'de> Visitor<'de> for DataVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("data")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut data = Vec::new();
            while let Some(byte) = seq.next_element::<u8>()? {
                data.put_u8(byte);
            }
            Ok(data)
        }
    }

    deserializer.deserialize_any(DataVisitor)
}

/// Deserialization of `Vec<u8>` using bulk byte reads instead of element-by-element.
/// Use with `#[serde(deserialize_with = "crate::de::vec_bytes")]` on `Vec<u8>` fields.
pub fn vec_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct VecBytesVisitor;

    impl<'de> Visitor<'de> for VecBytesVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("byte array")
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_vec())
        }
    }

    deserializer.deserialize_byte_buf(VecBytesVisitor)
}

/// Deserialization of `Bytes` from length-prefixed byte data.
/// Use with `#[serde(deserialize_with = "crate::de::bytes_deserialize")]` on `Bytes` fields.
pub fn bytes_deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BytesVisitor;

    impl<'de> Visitor<'de> for BytesVisitor {
        type Value = Bytes;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("byte array")
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Bytes::from(v))
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Bytes::copy_from_slice(v))
        }
    }

    deserializer.deserialize_byte_buf(BytesVisitor)
}

impl<'de> serde::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.input.len();
        visitor.visit_seq(SeqDeserializer {
            de: self,
            len: Some(len),
        })
    }

    fn deserialize_bool<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("bool not supported".to_owned()))
    }

    fn deserialize_i8<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("i8 not supported".to_owned()))
    }

    fn deserialize_i16<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("i16 not supported".to_owned()))
    }

    fn deserialize_i32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("i32 not supported".to_owned()))
    }

    fn deserialize_i64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("i64 not supported".to_owned()))
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.input.try_get_u8()?)
    }

    fn deserialize_u16<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("u16 not supported".to_owned()))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(self.input.try_get_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(self.input.try_get_u64()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("f32 not supported".to_owned()))
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("f64 not supported".to_owned()))
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("char not supported".to_owned()))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.input.try_get_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.input.try_get_string()?)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_byte_buf(self.input.try_get_bytes()?.to_vec())
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let bytes = self.input.try_get_bytes()?;
        // Pass owned Vec to visitor so it can take ownership
        visitor.visit_byte_buf(bytes.to_vec())
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("option not supported".to_owned()))
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.input.try_get_u32()? as usize;
        visitor.visit_seq(SeqDeserializer {
            de: self,
            len: Some(len),
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqDeserializer {
            de: self,
            len: Some(len),
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(MapDeserializer { de: self })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("identifier not supported".to_owned()))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::BadMessage("ignored any not supported".to_owned()))
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct SeqDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    len: Option<usize>,
}

impl<'de> SeqAccess<'de> for SeqDeserializer<'_, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        if self.len == Some(0) {
            return Ok(None);
        }

        if let Some(len) = self.len.as_mut() {
            *len -= 1;
        }

        seed.deserialize(&mut *self.de).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        self.len
    }
}

struct MapDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de> MapAccess<'de> for MapDeserializer<'_, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        if self.de.input.remaining() == 0 {
            return Ok(None);
        }

        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

impl<'de> VariantAccess<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        use serde::Deserializer;
        self.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        use crate::serde::Deserializer;
        self.deserialize_tuple(fields.len(), visitor)
    }
}

impl<'de> EnumAccess<'de> for &mut Deserializer<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let v = IntoDeserializer::<Self::Error>::into_deserializer(self.input.try_get_u32()?);
        Ok((seed.deserialize(v)?, self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ser;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct BytesField {
        id: u32,
        #[serde(deserialize_with = "bytes_deserialize")]
        #[serde(serialize_with = "crate::ser::bytes_serialize")]
        data: Bytes,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct VecBytesField {
        id: u32,
        #[serde(deserialize_with = "vec_bytes")]
        #[serde(serialize_with = "crate::ser::vec_serialize")]
        data: Vec<u8>,
    }

    #[test]
    fn bytes_deserialize_roundtrip() {
        let original = BytesField {
            id: 42,
            data: Bytes::from_static(b"test data"),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: BytesField = from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized, original);
    }

    #[test]
    fn bytes_deserialize_empty() {
        let original = BytesField {
            id: 1,
            data: Bytes::new(),
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: BytesField = from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.data.len(), 0);
    }

    #[test]
    fn vec_bytes_roundtrip() {
        let original = VecBytesField {
            id: 99,
            data: vec![1, 2, 3, 4, 5],
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: VecBytesField = from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.data, original.data);
    }

    #[test]
    fn vec_bytes_empty() {
        let original = VecBytesField {
            id: 0,
            data: vec![],
        };

        let serialized = ser::to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: VecBytesField = from_bytes(&mut bytes).expect("deserialize failed");

        assert!(deserialized.data.is_empty());
    }
}
