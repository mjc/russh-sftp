use bytes::{BufMut, Bytes, BytesMut};
use serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};

use crate::error::Error;

pub struct Serializer<'a> {
    output: &'a mut BytesMut,
}

fn checked_u32_len(len: usize) -> Result<u32, Error> {
    u32::try_from(len).map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))
}

/// Converting type to bytes according to protocol
pub fn to_bytes<T>(value: &T) -> Result<Bytes, Error>
where
    T: serde::Serialize + ?Sized,
{
    let mut output = BytesMut::new();
    {
        let mut serializer = Serializer {
            output: &mut output,
        };
        value.serialize(&mut serializer)?;
    }
    Ok(output.freeze())
}

/// Serialize directly into an existing BytesMut buffer
pub fn to_bytes_into<T>(value: &T, output: &mut BytesMut) -> Result<(), Error>
where
    T: serde::Serialize + ?Sized,
{
    let checkpoint = output.len();
    let result = {
        let mut serializer = Serializer { output };
        value.serialize(&mut serializer)
    };

    if let Err(err) = result {
        output.truncate(checkpoint);
        return Err(err);
    }

    Ok(())
}

/// Serialization of a [`Vec`] without length.
pub fn data_serialize<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut seq = serializer.serialize_seq(None)?;
    for byte in data {
        seq.serialize_element(byte)?;
    }
    seq.end()
}

/// Serialization of `Vec<u8>` with a length prefix.
pub fn vec_serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(data)
}

/// Serialization of [`Bytes`] with length prefix.
pub fn bytes_serialize<S>(data: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(data)
}

macro_rules! unsupported_serialize {
    ($($name:ident($ty:ty)),+ $(,)?) => {
        $(
            fn $name(self, _v: $ty) -> Result<Self::Ok, Self::Error> {
                Err(Error::BadMessage(concat!(stringify!($ty), " not supported").to_owned()))
            }
        )+
    };
}

impl<'a, 'b> serde::Serializer for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = &'a mut Serializer<'b>;
    type SerializeTuple = &'a mut Serializer<'b>;
    type SerializeTupleStruct = &'a mut Serializer<'b>;
    type SerializeTupleVariant = &'a mut Serializer<'b>;
    type SerializeMap = &'a mut Serializer<'b>;
    type SerializeStruct = &'a mut Serializer<'b>;
    type SerializeStructVariant = &'a mut Serializer<'b>;

    unsupported_serialize!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64)
    );

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.output.put_u8(v);
        Ok(())
    }

    unsupported_serialize!(serialize_u16(u16));

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.output.put_u32(v);
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.output.put_u64(v);
        Ok(())
    }

    unsupported_serialize!(serialize_f32(f32), serialize_f64(f64), serialize_char(char));

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let bytes = v.as_bytes();
        self.output.put_u32(checked_u32_len(bytes.len())?);
        self.output.put_slice(bytes);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.output.put_u32(checked_u32_len(v.len())?);
        self.output.put_slice(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::BadMessage("unit not supported".to_owned()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::BadMessage("unit struct not supported".to_owned()))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_u32(variant_index)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        if let Some(len) = len {
            self.output.put_u32(checked_u32_len(len)?);
        }

        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::BadMessage("struct variant not supported".to_owned()))
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl SerializeSeq for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeMap for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTuple for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeStruct for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeStructVariant for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTupleStruct for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTupleVariant for &mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::de;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct BytesStruct {
        id: u32,
        #[serde(deserialize_with = "crate::de::bytes_deserialize")]
        #[serde(serialize_with = "bytes_serialize")]
        data: Bytes,
    }

    #[test]
    fn bytes_serialize_produces_length_prefix() {
        let value = BytesStruct {
            id: 1,
            data: Bytes::from_static(b"abc"),
        };

        let serialized = to_bytes(&value).expect("serialize failed");

        // Expected format: u32 id (4 bytes) + u32 length (4 bytes) + data (3 bytes)
        assert_eq!(serialized.len(), 4 + 4 + 3);
        // Check length prefix is 3
        assert_eq!(&serialized[4..8], &[0, 0, 0, 3]);
        // Check data
        assert_eq!(&serialized[8..], b"abc");
    }

    #[test]
    fn bytes_serialize_empty() {
        let value = BytesStruct {
            id: 42,
            data: Bytes::new(),
        };

        let serialized = to_bytes(&value).expect("serialize failed");

        // Expected: u32 id + u32 length (0)
        assert_eq!(serialized.len(), 4 + 4);
        // Check length prefix is 0
        assert_eq!(&serialized[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn bytes_serialize_roundtrip() {
        let original = BytesStruct {
            id: 123,
            data: Bytes::from(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };

        let serialized = to_bytes(&original).expect("serialize failed");
        let mut bytes = serialized;
        let deserialized: BytesStruct = de::from_bytes(&mut bytes).expect("deserialize failed");

        assert_eq!(deserialized, original);
    }

    #[test]
    fn to_bytes_into_appends() {
        let value = 42u32;
        let mut buf = BytesMut::from(&[0xFFu8, 0xFF][..]);

        to_bytes_into(&value, &mut buf).expect("serialize failed");

        // Should have original 2 bytes + 4 bytes for u32
        assert_eq!(buf.len(), 6);
        assert_eq!(&buf[..2], &[0xFF, 0xFF]);
        assert_eq!(&buf[2..], &[0, 0, 0, 42]);
    }

    #[test]
    fn checked_u32_len_rejects_oversized_lengths() {
        let err = checked_u32_len(u32::MAX as usize + 1).expect_err("length should overflow u32");
        assert!(matches!(err, Error::BadMessage(_)));
        assert_eq!(err.to_string(), "Bad message: length exceeds u32::MAX");
    }

    #[test]
    fn serialize_seq_rejects_oversized_lengths() {
        let mut output = BytesMut::new();
        let mut serializer = Serializer {
            output: &mut output,
        };

        let err =
            match serde::Serializer::serialize_seq(&mut serializer, Some(u32::MAX as usize + 1)) {
                Ok(_) => panic!("sequence length should overflow u32"),
                Err(err) => err,
            };

        assert!(matches!(err, Error::BadMessage(_)));
        assert_eq!(err.to_string(), "Bad message: length exceeds u32::MAX");
    }

    #[derive(Debug)]
    struct PartialThenError;

    impl Serialize for PartialThenError {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_u32(7)?;
            Err(serde::ser::Error::custom("boom"))
        }
    }

    #[test]
    fn to_bytes_into_preserves_buffer_on_error() {
        let mut buf = BytesMut::from(&[0xAAu8, 0xBB][..]);

        let err = to_bytes_into(&PartialThenError, &mut buf).expect_err("serialize should fail");

        assert_eq!(err.to_string(), "Bad message: boom");
        assert_eq!(&buf[..], &[0xAA, 0xBB]);
    }
}
