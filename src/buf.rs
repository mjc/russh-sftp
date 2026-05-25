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
    fn try_get_bytes_reads_length_prefixed_payloads() {
        for (input, expected) in [
            (
                &[0, 0, 0, 5, b'h', b'e', b'l', b'l', b'o'][..],
                &b"hello"[..],
            ),
            (&[0, 0, 0, 0][..], &b""[..]),
        ] {
            let mut buf = Bytes::from_static(input);
            let result = buf.try_get_bytes().unwrap();
            assert_eq!(result.as_ref(), expected);
        }
    }

    #[test]
    fn try_get_bytes_rejects_truncated_payloads() {
        for input in [&[0, 0, 0, 10, b'a', b'b', b'c'][..], &[0, 0][..]] {
            let mut buf = Bytes::from_static(input);
            assert!(matches!(buf.try_get_bytes(), Err(Error::BadMessage(_))));
        }
    }

    #[test]
    fn try_get_string_reads_length_prefixed_payloads() {
        for (input, expected) in [
            (&[0, 0, 0, 5, b'w', b'o', b'r', b'l', b'd'][..], "world"),
            (&[0, 0, 0, 0][..], ""),
        ] {
            let mut buf = Bytes::from_static(input);
            assert_eq!(buf.try_get_string().unwrap(), expected);
        }
    }

    #[test]
    fn try_get_string_invalid_utf8_uses_replacement() {
        let mut buf = Bytes::from_static(&[0, 0, 0, 2, 0xFF, 0xFE]);
        let result = buf.try_get_string().unwrap();

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
