use bytes::{Buf, Bytes};

use crate::error::Error;

pub trait TryBuf: Buf {
    fn try_get_bytes(&mut self) -> Result<Bytes, Error>;
    fn try_get_string(&mut self) -> Result<String, Error>;
}

impl<T: Buf> TryBuf for T {
    fn try_get_bytes(&mut self) -> Result<Bytes, Error> {
        let len = self.try_get_u32()? as usize;
        if self.remaining() < len {
            return Err(Error::BadMessage("no remaining for bytes".to_owned()));
        }

        // Zero-copy slice if input is Bytes, otherwise copies
        Ok(self.copy_to_bytes(len))
    }

    fn try_get_string(&mut self) -> Result<String, Error> {
        let bytes = self.try_get_bytes()?;
        // Use lossy conversion to maintain compatibility with SFTP implementations
        // that may send invalid UTF-8 in file names or other string fields
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn try_get_bytes_success() {
        // Length-prefixed bytes: 4-byte length (5) + 5 bytes of data
        let mut buf = Bytes::from_static(&[0, 0, 0, 5, b'h', b'e', b'l', b'l', b'o']);
        let result = buf.try_get_bytes().unwrap();
        assert_eq!(result.as_ref(), b"hello");
        assert_eq!(buf.remaining(), 0);
    }

    #[test]
    fn try_get_bytes_empty() {
        // Length-prefixed empty bytes
        let mut buf = Bytes::from_static(&[0, 0, 0, 0]);
        let result = buf.try_get_bytes().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn try_get_bytes_insufficient_length() {
        // Claims 10 bytes but only has 3
        let mut buf = Bytes::from_static(&[0, 0, 0, 10, b'a', b'b', b'c']);
        let result = buf.try_get_bytes();
        assert!(result.is_err());
    }

    #[test]
    fn try_get_bytes_no_length() {
        // Not enough bytes for length prefix
        let mut buf = Bytes::from_static(&[0, 0]);
        let result = buf.try_get_bytes();
        assert!(matches!(result, Err(Error::BadMessage(_))));
    }

    #[test]
    fn try_get_string_success() {
        let mut buf = Bytes::from_static(&[0, 0, 0, 5, b'w', b'o', b'r', b'l', b'd']);
        let result = buf.try_get_string().unwrap();
        assert_eq!(result, "world");
    }

    #[test]
    fn try_get_string_empty() {
        let mut buf = Bytes::from_static(&[0, 0, 0, 0]);
        let result = buf.try_get_string().unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn try_get_string_invalid_utf8_uses_replacement() {
        // Invalid UTF-8 sequence - should use replacement character (lossy)
        let mut buf = Bytes::from_static(&[0, 0, 0, 2, 0xFF, 0xFE]);
        let result = buf.try_get_string().unwrap();
        // Lossy conversion replaces invalid bytes with U+FFFD
        assert!(result.contains('\u{FFFD}'));
    }

    #[test]
    fn try_get_bytes_works_with_bytes_mut() {
        // Verify it works with BytesMut too
        let mut buf = BytesMut::from(&[0u8, 0, 0, 3, b'a', b'b', b'c'][..]);
        let result = buf.try_get_bytes().unwrap();
        assert_eq!(result.as_ref(), b"abc");
    }
}
