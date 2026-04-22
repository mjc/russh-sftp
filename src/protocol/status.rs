use bytes::Buf;
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
    pub error_message: String,
    pub language_tag: String,
}

impl Status {
    pub fn from_bytes<B: Buf + TryBuf>(input: &mut B) -> Result<Self, ProtocolError> {
        Ok(Self {
            id: input.try_get_u32()?,
            status_code: StatusCode::try_from(input.try_get_u32()?)?,
            error_message: input.try_get_string()?,
            language_tag: input.try_get_string()?,
        })
    }
}

impl_request_id!(Status);
impl_packet_for!(Status);
