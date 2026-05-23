use bytes::{Buf, BufMut, BytesMut};
use std::borrow::Cow;
use thiserror::Error;

use super::{impl_packet_for, impl_request_id, Packet, RequestId};
use crate::{buf::TryBuf, error::Error as ProtocolError};

/// Error Codes for SSH_FXP_STATUS
#[derive(Debug, Error, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StatusCode {
    /// Indicates successful completion of the operation.
    #[error("Ok")]
    Ok = 0,
    /// Indicates end-of-file condition; for SSH_FX_READ it means that no more data is available in the file,
    /// and for SSH_FX_READDIR it indicates that no more files are contained in the directory.
    #[error("Eof")]
    Eof = 1,
    /// A reference is made to a file which should exist but doesn't.
    #[error("No such file")]
    NoSuchFile = 2,
    /// Authenticated user does not have sufficient permissions to perform the operation.
    #[error("Permission denied")]
    PermissionDenied = 3,
    /// A generic catch-all error message;
    /// it should be returned if an error occurs for which there is no more specific error code defined.
    #[error("Failure")]
    Failure = 4,
    /// May be returned if a badly formatted packet or protocol incompatibility is detected.
    #[error("Bad message")]
    BadMessage = 5,
    /// A pseudo-error which indicates that the client has no connection to the server
    /// (it can only be generated locally by the client, and MUST NOT be returned by servers).
    #[error("No connection")]
    NoConnection = 6,
    /// A pseudo-error which indicates that the connection to the server has been lost
    /// (it can only be generated locally by the client, and MUST NOT be returned by servers).
    #[error("Connection lost")]
    ConnectionLost = 7,
    /// Indicates that an attempt was made to perform an operation which is not supported for the server
    /// (it may be generated locally by the client if e.g. the version number exchange indicates that a required feature is not supported by the server,
    /// or it may be returned by the server if the server does not implement an operation).
    #[error("Operation unsupported")]
    OpUnsupported = 8,
}

impl TryFrom<u32> for StatusCode {
    type Error = ProtocolError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Eof),
            2 => Ok(Self::NoSuchFile),
            3 => Ok(Self::PermissionDenied),
            4 => Ok(Self::Failure),
            5 => Ok(Self::BadMessage),
            6 => Ok(Self::NoConnection),
            7 => Ok(Self::ConnectionLost),
            8 => Ok(Self::OpUnsupported),
            _ => Err(ProtocolError::BadMessage(format!(
                "unknown status code {value}"
            ))),
        }
    }
}

/// Implementation for SSH_FXP_STATUS as defined in the specification draft
/// <https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02#section-7>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub id: u32,
    pub status_code: StatusCode,
    pub error_message: Cow<'static, str>,
    pub language_tag: Cow<'static, str>,
}

impl Status {
    fn decode_common_string<B: Buf + TryBuf>(input: &mut B) -> Result<Cow<'static, str>, ProtocolError> {
        let bytes = input.try_get_bytes()?;

        if bytes.as_ref() == b"Ok" {
            return Ok(Cow::Borrowed("Ok"));
        }

        if bytes.as_ref() == b"en-US" {
            return Ok(Cow::Borrowed("en-US"));
        }

        Ok(Cow::Owned(String::from_utf8_lossy(&bytes).into_owned()))
    }

    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, ProtocolError> {
        Ok(Self {
            id: input.try_get_u32()?,
            status_code: StatusCode::try_from(input.try_get_u32()?)?,
            error_message: Self::decode_common_string(input)?,
            language_tag: Self::decode_common_string(input)?,
        })
    }

    pub(crate) fn serialize_into(&self, output: &mut BytesMut) -> Result<(), ProtocolError> {
        let error_message = self.error_message.as_bytes();
        let language_tag = self.language_tag.as_bytes();

        output.put_u32(self.id);
        output.put_u32(self.status_code as u32);
        output.put_u32(
            u32::try_from(error_message.len())
                .map_err(|_| ProtocolError::BadMessage("length exceeds u32::MAX".to_owned()))?,
        );
        output.put_slice(error_message);
        output.put_u32(
            u32::try_from(language_tag.len())
                .map_err(|_| ProtocolError::BadMessage("length exceeds u32::MAX".to_owned()))?,
        );
        output.put_slice(language_tag);

        Ok(())
    }
}

impl_request_id!(Status);
impl_packet_for!(Status);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_into_matches_serde_output() {
        let status = Status {
            id: 11,
            status_code: StatusCode::Ok,
            error_message: Cow::Borrowed("Ok"),
            language_tag: Cow::Borrowed("en-US"),
        };

        let expected = crate::ser::to_bytes(&status).expect("serialize with serde");
        let mut actual = BytesMut::new();
        status.serialize_into(&mut actual).expect("serialize manually");

        assert_eq!(actual.freeze(), expected);
    }

    #[test]
    fn from_bytes_borrows_common_ok_strings() {
        let mut buf = bytes::BytesMut::new();
        buf.put_u32(7);
        buf.put_u32(StatusCode::Ok as u32);
        buf.put_u32(2);
        buf.extend_from_slice(b"Ok");
        buf.put_u32(5);
        buf.extend_from_slice(b"en-US");

        let mut bytes = buf.freeze();
        let status = Status::from_bytes(&mut bytes).expect("parse status");

        assert!(matches!(status.error_message, Cow::Borrowed("Ok")));
        assert!(matches!(status.language_tag, Cow::Borrowed("en-US")));
    }
}
