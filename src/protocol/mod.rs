mod attrs;
mod close;
mod data;
mod extended;
mod file;
mod file_attrs;
mod fsetstat;
mod fstat;
mod handle;
mod init;
mod lstat;
mod mkdir;
mod name;
mod open;
mod opendir;
mod read;
mod readdir;
mod readlink;
mod realpath;
mod remove;
mod rename;
mod rmdir;
mod setstat;
mod stat;
mod status;
mod symlink;
mod version;
mod write;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::borrow::Cow;
use std::io::IoSlice;

use crate::{buf::TryBuf, error::Error, ser, utils::MAX_PACKET_SIZE};

pub use self::{
    attrs::Attrs,
    close::Close,
    data::{Data, DataPayload},
    extended::{Extended, ExtendedReply},
    file::File,
    file_attrs::{
        FileAttr, FileAttributes, FileMode, FilePermissionFlags, FilePermissions, FileType,
    },
    fsetstat::FSetStat,
    fstat::Fstat,
    handle::Handle,
    init::Init,
    lstat::Lstat,
    mkdir::MkDir,
    name::Name,
    open::{Open, OpenFlags},
    opendir::OpenDir,
    read::Read,
    readdir::ReadDir,
    readlink::ReadLink,
    realpath::RealPath,
    remove::Remove,
    rename::Rename,
    rmdir::RmDir,
    setstat::SetStat,
    stat::Stat,
    status::{Status, StatusCode},
    symlink::Symlink,
    version::Version,
    write::Write,
};

pub const VERSION: u32 = 3;

const SSH_FXP_INIT: u8 = 1;
const SSH_FXP_VERSION: u8 = 2;
const SSH_FXP_OPEN: u8 = 3;
const SSH_FXP_CLOSE: u8 = 4;
const SSH_FXP_READ: u8 = 5;
const SSH_FXP_WRITE: u8 = 6;
const SSH_FXP_LSTAT: u8 = 7;
const SSH_FXP_FSTAT: u8 = 8;
const SSH_FXP_SETSTAT: u8 = 9;
const SSH_FXP_FSETSTAT: u8 = 10;
const SSH_FXP_OPENDIR: u8 = 11;
const SSH_FXP_READDIR: u8 = 12;
const SSH_FXP_REMOVE: u8 = 13;
const SSH_FXP_MKDIR: u8 = 14;
const SSH_FXP_RMDIR: u8 = 15;
const SSH_FXP_REALPATH: u8 = 16;
const SSH_FXP_STAT: u8 = 17;
const SSH_FXP_RENAME: u8 = 18;
const SSH_FXP_READLINK: u8 = 19;
const SSH_FXP_SYMLINK: u8 = 20;

const SSH_FXP_STATUS: u8 = 101;
const SSH_FXP_HANDLE: u8 = 102;
const SSH_FXP_DATA: u8 = 103;
const SSH_FXP_NAME: u8 = 104;
const SSH_FXP_ATTRS: u8 = 105;

const SSH_FXP_EXTENDED: u8 = 200;
const SSH_FXP_EXTENDED_REPLY: u8 = 201;

pub(crate) trait RequestId: Sized {
    fn get_request_id(&self) -> u32;
}

macro_rules! impl_request_id {
    ($packet:ty) => {
        impl RequestId for $packet {
            fn get_request_id(&self) -> u32 {
                self.id
            }
        }
    };
}

macro_rules! impl_packet_for {
    ($name:ident) => {
        impl From<$name> for Packet {
            fn from(input: $name) -> Self {
                Self::$name(input)
            }
        }
    };
}

pub(crate) use impl_packet_for;
pub(crate) use impl_request_id;

#[derive(Debug)]
pub enum Packet {
    Init(Init),
    Version(Version),
    Open(Open),
    Close(Close),
    Read(Read),
    Write(Write),
    Lstat(Lstat),
    Fstat(Fstat),
    SetStat(SetStat),
    FSetStat(FSetStat),
    OpenDir(OpenDir),
    ReadDir(ReadDir),
    Remove(Remove),
    MkDir(MkDir),
    RmDir(RmDir),
    RealPath(RealPath),
    Stat(Stat),
    Rename(Rename),
    ReadLink(ReadLink),
    Symlink(Symlink),
    Status(Status),
    Handle(Handle),
    Data(Data),
    Name(Name),
    Attrs(Attrs),
    Extended(Extended),
    ExtendedReply(ExtendedReply),
}

impl Packet {
    pub fn get_request_id(&self) -> u32 {
        match self {
            Self::Open(open) => open.get_request_id(),
            Self::Close(close) => close.get_request_id(),
            Self::Read(read) => read.get_request_id(),
            Self::Write(write) => write.get_request_id(),
            Self::Lstat(lstat) => lstat.get_request_id(),
            Self::Fstat(fstat) => fstat.get_request_id(),
            Self::SetStat(setstat) => setstat.get_request_id(),
            Self::FSetStat(fsetstat) => fsetstat.get_request_id(),
            Self::OpenDir(opendir) => opendir.get_request_id(),
            Self::ReadDir(readdir) => readdir.get_request_id(),
            Self::Remove(remove) => remove.get_request_id(),
            Self::MkDir(mkdir) => mkdir.get_request_id(),
            Self::RmDir(rmdir) => rmdir.get_request_id(),
            Self::RealPath(realpath) => realpath.get_request_id(),
            Self::Stat(stat) => stat.get_request_id(),
            Self::Rename(rename) => rename.get_request_id(),
            Self::ReadLink(readlink) => readlink.get_request_id(),
            Self::Symlink(symlink) => symlink.get_request_id(),
            Self::Extended(extended) => extended.get_request_id(),
            _ => 0,
        }
    }

    pub fn status(
        id: u32,
        status_code: StatusCode,
        msg: impl Into<Cow<'static, str>>,
        tag: impl Into<Cow<'static, str>>,
    ) -> Self {
        Packet::Status(Status {
            id,
            status_code,
            error_message: msg.into(),
            language_tag: tag.into(),
        })
    }

    pub fn error(id: u32, status_code: StatusCode) -> Self {
        Self::status(id, status_code, status_code.to_string(), "en-US")
    }
}

impl TryFrom<&mut Bytes> for Packet {
    type Error = Error;

    fn try_from(bytes: &mut Bytes) -> Result<Self, Self::Error> {
        try_from_buf(bytes)
    }
}

impl TryFrom<&mut BytesMut> for Packet {
    type Error = Error;

    fn try_from(bytes: &mut BytesMut) -> Result<Self, Self::Error> {
        try_from_buf(bytes)
    }
}

fn try_from_buf<B>(bytes: &mut B) -> Result<Packet, Error>
where
    B: Buf + TryBuf,
{
    let r#type = bytes.try_get_u8()?;
    debug!("packet type {}", r#type);

    let request = match r#type {
        SSH_FXP_INIT => Packet::Init(Init::from_bytes(bytes)?),
        SSH_FXP_VERSION => Packet::Version(Version::from_bytes(bytes)?),
        SSH_FXP_OPEN => Packet::Open(Open::from_bytes(bytes)?),
        SSH_FXP_CLOSE => Packet::Close(Close::from_bytes(bytes)?),
        // Manual deserialization for consistency with Write/Data
        SSH_FXP_READ => Packet::Read(Read::from_bytes(bytes)?),
        // Zero-copy deserialization - bypasses serde to avoid Vec allocation
        SSH_FXP_WRITE => Packet::Write(Write::from_bytes(bytes)?),
        SSH_FXP_LSTAT => Packet::Lstat(Lstat::from_bytes(bytes)?),
        SSH_FXP_FSTAT => Packet::Fstat(Fstat::from_bytes(bytes)?),
        SSH_FXP_SETSTAT => Packet::SetStat(SetStat::from_bytes(bytes)?),
        SSH_FXP_FSETSTAT => Packet::FSetStat(FSetStat::from_bytes(bytes)?),
        SSH_FXP_OPENDIR => Packet::OpenDir(OpenDir::from_bytes(bytes)?),
        SSH_FXP_READDIR => Packet::ReadDir(ReadDir::from_bytes(bytes)?),
        SSH_FXP_REMOVE => Packet::Remove(Remove::from_bytes(bytes)?),
        SSH_FXP_MKDIR => Packet::MkDir(MkDir::from_bytes(bytes)?),
        SSH_FXP_RMDIR => Packet::RmDir(RmDir::from_bytes(bytes)?),
        SSH_FXP_REALPATH => Packet::RealPath(RealPath::from_bytes(bytes)?),
        SSH_FXP_STAT => Packet::Stat(Stat::from_bytes(bytes)?),
        SSH_FXP_RENAME => Packet::Rename(Rename::from_bytes(bytes)?),
        SSH_FXP_READLINK => Packet::ReadLink(ReadLink::from_bytes(bytes)?),
        SSH_FXP_SYMLINK => Packet::Symlink(Symlink::from_bytes(bytes)?),
        SSH_FXP_STATUS => Packet::Status(Status::from_bytes(bytes)?),
        SSH_FXP_HANDLE => Packet::Handle(Handle::from_bytes(bytes)?),
        // Zero-copy deserialization - bypasses serde to avoid Vec allocation
        SSH_FXP_DATA => Packet::Data(Data::from_bytes(bytes)?),
        SSH_FXP_NAME => Packet::Name(Name::from_bytes(bytes)?),
        SSH_FXP_ATTRS => Packet::Attrs(Attrs::from_bytes(bytes)?),
        SSH_FXP_EXTENDED => Packet::Extended(Extended::from_bytes(bytes)?),
        SSH_FXP_EXTENDED_REPLY => Packet::ExtendedReply(ExtendedReply::from_bytes(bytes)?),
        _ => return Err(Error::BadMessage("unknown type".to_owned())),
    };

    Ok(request)
}

macro_rules! serialize_packet {
    ($($variant:ident => $type_const:expr),+ $(,)?) => {
        |packet: Packet, bytes: &mut BytesMut| -> Result<(), Error> {
            match packet {
                $(
                    Packet::$variant(v) => {
                        bytes.put_u8($type_const);
                        ser::to_bytes_into(&v, bytes)?;
                    }
                )+
                _ => unreachable!("packet variant should have been handled before serializer"),
            }
            Ok(())
        }
    };
}

pub enum SerializedPacket {
    Contiguous(Bytes),
    Split { header: Bytes, data: DataPayload },
}

impl SerializedPacket {
    pub async fn write_to<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
    ) -> Result<(), std::io::Error> {
        use tokio::io::AsyncWriteExt;

        match self {
            Self::Contiguous(bytes) => {
                stream.write_all(bytes).await?;
            }
            Self::Split { header, data } => {
                let mut header_offset = 0;
                let mut data_offset = 0;

                while header_offset < header.len() || data_offset < data.len() {
                    let buffers = [
                        IoSlice::new(&header[header_offset..]),
                        IoSlice::new(&data.as_ref()[data_offset..]),
                    ];

                    let written = stream.write_vectored(&buffers).await?;
                    if written == 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "failed to write entire buffer",
                        ));
                    }

                    let mut remaining = written;
                    if header_offset < header.len() {
                        let header_written = remaining.min(header.len() - header_offset);
                        header_offset += header_written;
                        remaining -= header_written;
                    }
                    if remaining > 0 {
                        data_offset += remaining;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Serialize a packet into an existing buffer, returning owned `Bytes`.
/// The buffer is cleared and reused while serializing, then cloned into the
/// returned `Bytes`.
pub fn serialize_packet_into(packet: Packet, buf: &mut BytesMut) -> Result<Bytes, Error> {
    serialize_packet_into_buf(packet, buf)?;
    Ok(buf.clone().freeze())
}

fn checked_packet_length(length: usize) -> Result<u32, Error> {
    if length > MAX_PACKET_SIZE {
        return Err(Error::BadMessage(format!(
            "length {length} exceeds maximum {MAX_PACKET_SIZE}"
        )));
    }

    u32::try_from(length).map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))
}

fn checked_add_len(lhs: usize, rhs: usize) -> Result<usize, Error> {
    lhs.checked_add(rhs)
        .ok_or_else(|| Error::BadMessage("length overflow".to_owned()))
}

fn checked_write_packet_payload_len(write: &Write) -> Result<usize, Error> {
    let length = checked_add_len(1, 4)?;
    let length = checked_add_len(length, 4)?;
    let length = checked_add_len(length, write.handle.len())?;
    let length = checked_add_len(length, 8)?;
    let length = checked_add_len(length, 4)?;
    let length = checked_add_len(length, write.data.len())?;
    checked_packet_length(length)?;
    Ok(length)
}

fn checked_data_packet_payload_len(data: &Data) -> Result<usize, Error> {
    let length = checked_add_len(1, 4)?;
    let length = checked_add_len(length, 4)?;
    let length = checked_add_len(length, data.data.len())?;
    checked_packet_length(length)?;
    Ok(length)
}

pub(crate) fn serialize_read_packet(
    id: u32,
    handle: &Bytes,
    offset: u64,
    len: u32,
) -> Result<Bytes, Error> {
    let payload_len = checked_add_len(1, 4)?;
    let payload_len = checked_add_len(payload_len, 4)?;
    let payload_len = checked_add_len(payload_len, handle.len())?;
    let payload_len = checked_add_len(payload_len, 8)?;
    let payload_len = checked_add_len(payload_len, 4)?;
    let packet_len = checked_packet_length(payload_len)?;

    let mut buf = BytesMut::with_capacity(4 + payload_len);
    buf.put_u32(packet_len);
    buf.put_u8(SSH_FXP_READ);
    buf.put_u32(id);
    buf.put_u32(
        u32::try_from(handle.len())
            .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?,
    );
    buf.put_slice(handle);
    buf.put_u64(offset);
    buf.put_u32(len);
    Ok(buf.freeze())
}

pub(crate) fn serialize_write_packet(
    id: u32,
    handle: &Bytes,
    offset: u64,
    data: &Bytes,
) -> Result<Bytes, Error> {
    let write = Write {
        id,
        handle: handle.clone(),
        offset,
        data: data.clone(),
    };
    let payload_len = checked_write_packet_payload_len(&write)?;
    let packet_len = checked_packet_length(payload_len)?;

    let mut buf = BytesMut::with_capacity(4 + payload_len);
    buf.put_u32(packet_len);
    buf.put_u8(SSH_FXP_WRITE);
    buf.put_u32(id);
    buf.put_u32(
        u32::try_from(handle.len())
            .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?,
    );
    buf.put_slice(handle);
    buf.put_u64(offset);
    buf.put_u32(
        u32::try_from(data.len())
            .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?,
    );
    buf.put_slice(data);
    Ok(buf.freeze())
}

pub(crate) fn serialize_packet_split(
    packet: Packet,
    buf: &mut BytesMut,
) -> Result<SerializedPacket, Error> {
    if let Packet::Data(mut data) = packet {
        let payload_len = checked_data_packet_payload_len(&data)?;
        let packet_len = checked_packet_length(payload_len)?;
        let data_len = u32::try_from(data.data.len())
            .map_err(|_| Error::BadMessage("length exceeds u32::MAX".to_owned()))?;

        buf.clear();
        buf.reserve(13);
        buf.put_u32(packet_len);
        buf.put_u8(SSH_FXP_DATA);
        buf.put_u32(data.id);
        buf.put_u32(data_len);

        if data.data.try_prepend(buf) {
            buf.clear();
            return Ok(SerializedPacket::Split {
                header: Bytes::new(),
                data: data.data,
            });
        }

        return Ok(SerializedPacket::Split {
            header: buf.split().freeze(),
            data: data.data,
        });
    }

    Ok(SerializedPacket::Contiguous(serialize_packet_into(
        packet, buf,
    )?))
}

pub(crate) fn serialize_packet_into_buf(packet: Packet, buf: &mut BytesMut) -> Result<(), Error> {
    buf.clear();

    let result = (|| {
        // Estimate capacity based on packet type to avoid reallocations
        let capacity = match &packet {
            Packet::Write(w) => {
                let payload_len = checked_write_packet_payload_len(w)?;
                4 + payload_len
            }
            Packet::Data(d) => {
                let payload_len = checked_data_packet_payload_len(d)?;
                4 + payload_len
            }
            _ => 256,
        };
        buf.reserve(capacity);

        // Single buffer: [length:4][type:1][payload...]
        buf.put_u32(0); // placeholder for length

        match packet {
            Packet::Data(data) => {
                buf.put_u8(SSH_FXP_DATA);
                data.serialize_into(buf)?;
            }
            Packet::Status(status) => {
                buf.put_u8(SSH_FXP_STATUS);
                status.serialize_into(buf)?;
            }
            other => {
                let serializer = serialize_packet!(
                    Init => SSH_FXP_INIT,
                    Version => SSH_FXP_VERSION,
                    Open => SSH_FXP_OPEN,
                    Close => SSH_FXP_CLOSE,
                    Read => SSH_FXP_READ,
                    Write => SSH_FXP_WRITE,
                    Lstat => SSH_FXP_LSTAT,
                    Fstat => SSH_FXP_FSTAT,
                    SetStat => SSH_FXP_SETSTAT,
                    FSetStat => SSH_FXP_FSETSTAT,
                    OpenDir => SSH_FXP_OPENDIR,
                    ReadDir => SSH_FXP_READDIR,
                    Remove => SSH_FXP_REMOVE,
                    MkDir => SSH_FXP_MKDIR,
                    RmDir => SSH_FXP_RMDIR,
                    RealPath => SSH_FXP_REALPATH,
                    Stat => SSH_FXP_STAT,
                    Rename => SSH_FXP_RENAME,
                    ReadLink => SSH_FXP_READLINK,
                    Symlink => SSH_FXP_SYMLINK,
                    Handle => SSH_FXP_HANDLE,
                    Name => SSH_FXP_NAME,
                    Attrs => SSH_FXP_ATTRS,
                    Extended => SSH_FXP_EXTENDED,
                    ExtendedReply => SSH_FXP_EXTENDED_REPLY,
                );
                serializer(other, buf)?;
            }
        }

        // Patch length (excludes the 4-byte length field itself)
        let length = checked_packet_length(buf.len() - 4)?;
        buf[0..4].copy_from_slice(&length.to_be_bytes());

        Ok(())
    })();

    if result.is_err() {
        buf.clear();
    }

    result
}

impl TryFrom<Packet> for Bytes {
    type Error = Error;

    fn try_from(packet: Packet) -> Result<Self, Self::Error> {
        let mut buf = BytesMut::new();
        serialize_packet_into_buf(packet, &mut buf)?;
        Ok(buf.freeze())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BufMut;

    fn assert_packet_roundtrip(packet: Packet, check: impl FnOnce(Packet)) {
        let serialized: Bytes = packet.try_into().expect("serialize failed");
        let mut bytes = serialized.slice(4..);
        let parsed = Packet::try_from(&mut bytes).expect("deserialize failed");
        check(parsed);
    }

    #[test]
    fn packet_write_roundtrip() {
        let original = Write {
            id: 42,
            handle: Bytes::from_static(b"test-handle"),
            offset: 1024,
            data: Bytes::from_static(b"hello world"),
        };
        assert_packet_roundtrip(Packet::Write(original), |packet| {
            if let Packet::Write(write) = packet {
                assert_eq!(write.id, 42);
                assert_eq!(write.handle.as_ref(), b"test-handle");
                assert_eq!(write.offset, 1024);
                assert_eq!(write.data.as_ref(), b"hello world");
            } else {
                panic!("Expected Write packet");
            }
        });
    }

    #[test]
    fn packet_data_roundtrip() {
        let original = Data {
            id: 99,
            data: Bytes::from(vec![0xDE, 0xAD, 0xBE, 0xEF]).into(),
        };
        assert_packet_roundtrip(Packet::Data(original), |packet| {
            if let Packet::Data(data) = packet {
                assert_eq!(data.id, 99);
                assert_eq!(data.data.as_ref(), &[0xDE, 0xAD, 0xBE, 0xEF]);
            } else {
                panic!("Expected Data packet");
            }
        });
    }

    #[test]
    fn split_data_serialization_matches_contiguous() {
        let payload = vec![0xCAu8; 32 * 1024];

        let contiguous = Bytes::try_from(Packet::Data(Data {
            id: 42,
            data: Bytes::from(payload.clone()).into(),
        }))
        .expect("contiguous serialize failed");

        let mut buf = BytesMut::new();
        let split = serialize_packet_split(
            Packet::Data(Data {
                id: 42,
                data: Bytes::from(payload).into(),
            }),
            &mut buf,
        )
        .expect("split serialize failed");

        let reassembled = match split {
            SerializedPacket::Contiguous(_) => panic!("expected split packet"),
            SerializedPacket::Split { header, data } => {
                let mut combined = BytesMut::with_capacity(header.len() + data.len());
                combined.extend_from_slice(&header);
                combined.extend_from_slice(data.as_ref());
                combined.freeze()
            }
        };

        assert_eq!(reassembled, contiguous);
    }

    #[cfg(feature = "russh-channel-data")]
    #[test]
    fn split_data_serialization_uses_reusable_prefix_headroom() {
        use std::sync::Arc;

        struct DropRecycler;

        impl russh::ChannelDataRecycler for DropRecycler {
            fn recycle(&self, _data: Vec<u8>) {}
        }

        let mut backing = vec![0; 13];
        backing.extend_from_slice(b"payload");
        let reusable =
            russh::ReusableChannelData::try_new_with_range(backing, 13, 7, Arc::new(DropRecycler))
                .unwrap();
        let mut buf = BytesMut::new();

        let split = serialize_packet_split(
            Packet::Data(Data {
                id: 7,
                data: russh::ChannelData::Reusable(reusable).into(),
            }),
            &mut buf,
        )
        .expect("split serialize failed");

        match split {
            SerializedPacket::Split { header, data } => {
                assert!(header.is_empty());
                assert_eq!(
                    data.as_ref(),
                    &[
                        0,
                        0,
                        0,
                        16,
                        SSH_FXP_DATA,
                        0,
                        0,
                        0,
                        7,
                        0,
                        0,
                        0,
                        7,
                        b'p',
                        b'a',
                        b'y',
                        b'l',
                        b'o',
                        b'a',
                        b'd'
                    ]
                );
            }
            SerializedPacket::Contiguous(_) => panic!("expected split packet"),
        }
    }

    #[test]
    fn packet_write_large_data() {
        let large_data = vec![0xABu8; 64 * 1024]; // 64KB
        let original = Write {
            id: 1,
            handle: Bytes::from_static(b"big"),
            offset: 0,
            data: Bytes::from(large_data.clone()),
        };

        assert_packet_roundtrip(Packet::Write(original), |packet| {
            if let Packet::Write(write) = packet {
                assert_eq!(write.data.len(), 64 * 1024);
                assert_eq!(write.data.as_ref(), large_data.as_slice());
            } else {
                panic!("Expected Write packet");
            }
        });
    }

    #[test]
    fn packet_handle_roundtrip() {
        let original = Handle {
            id: 7,
            handle: Bytes::from_static(b"opaque-handle"),
        };

        assert_packet_roundtrip(Packet::Handle(original), |packet| {
            if let Packet::Handle(handle) = packet {
                assert_eq!(handle.id, 7);
                assert_eq!(handle.handle.as_ref(), b"opaque-handle");
            } else {
                panic!("Expected Handle packet");
            }
        });
    }

    #[test]
    fn packet_status_roundtrip() {
        let original = Status {
            id: 9,
            status_code: StatusCode::Failure,
            error_message: "failed".into(),
            language_tag: "en-US".into(),
        };

        assert_packet_roundtrip(Packet::Status(original), |packet| {
            if let Packet::Status(status) = packet {
                assert_eq!(status.id, 9);
                assert_eq!(status.status_code, StatusCode::Failure);
                assert_eq!(status.error_message, "failed");
                assert_eq!(status.language_tag, "en-US");
            } else {
                panic!("Expected Status packet");
            }
        });
    }

    #[test]
    fn packet_name_roundtrip() {
        let original = Name {
            id: 11,
            files: vec![
                File {
                    filename: "a.txt".to_string(),
                    longname: "-rw-r--r-- a.txt".to_string(),
                    attrs: FileAttributes {
                        size: Some(1),
                        uid: Some(1000),
                        user: None,
                        gid: Some(1001),
                        group: None,
                        permissions: Some(0o100644),
                        atime: Some(10),
                        mtime: Some(20),
                    },
                },
                File {
                    filename: "dir".to_string(),
                    longname: "drwxr-xr-x dir".to_string(),
                    attrs: FileAttributes {
                        permissions: Some(0o040755),
                        ..FileAttributes::empty()
                    },
                },
            ],
        };

        assert_packet_roundtrip(Packet::Name(original), |packet| {
            if let Packet::Name(name) = packet {
                assert_eq!(name.id, 11);
                assert_eq!(name.files.len(), 2);
                assert_eq!(name.files[0].filename, "a.txt");
                assert_eq!(name.files[0].attrs.size, Some(1));
                assert_eq!(name.files[1].filename, "dir");
                assert_eq!(name.files[1].attrs.permissions, Some(0o040755));
            } else {
                panic!("Expected Name packet");
            }
        });
    }

    #[test]
    fn packet_open_roundtrip() {
        let original = Open {
            id: 12,
            filename: "/tmp/file".to_string(),
            pflags: OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE,
            attrs: FileAttributes {
                size: Some(77),
                permissions: Some(0o100644),
                ..FileAttributes::empty()
            },
        };

        assert_packet_roundtrip(Packet::Open(original), |packet| {
            if let Packet::Open(open) = packet {
                assert_eq!(open.id, 12);
                assert_eq!(open.filename, "/tmp/file");
                assert!(open.pflags.contains(OpenFlags::READ));
                assert!(open.pflags.contains(OpenFlags::WRITE));
                assert!(open.pflags.contains(OpenFlags::CREATE));
                assert_eq!(open.attrs.size, Some(77));
                assert_eq!(open.attrs.permissions, Some(0o100644));
            } else {
                panic!("Expected Open packet");
            }
        });
    }

    #[test]
    fn packet_rename_roundtrip() {
        let original = Rename {
            id: 13,
            oldpath: "/old".to_string(),
            newpath: "/new".to_string(),
        };

        assert_packet_roundtrip(Packet::Rename(original), |packet| {
            if let Packet::Rename(rename) = packet {
                assert_eq!(rename.oldpath, "/old");
                assert_eq!(rename.newpath, "/new");
            } else {
                panic!("Expected Rename packet");
            }
        });
    }

    #[test]
    fn packet_init_roundtrip() {
        let mut extensions = std::collections::HashMap::new();
        extensions.insert("limits@openssh.com".to_string(), "1".to_string());
        extensions.insert("statvfs@openssh.com".to_string(), "2".to_string());

        assert_packet_roundtrip(
            Packet::Init(Init {
                version: VERSION,
                extensions: extensions.clone(),
            }),
            |packet| {
                if let Packet::Init(init) = packet {
                    assert_eq!(init.version, VERSION);
                    assert_eq!(init.extensions, extensions);
                } else {
                    panic!("Expected Init packet");
                }
            },
        );
    }

    #[test]
    fn packet_fsetstat_roundtrip() {
        let original = FSetStat {
            id: 21,
            handle: Bytes::from_static(b"opaque-handle"),
            attrs: FileAttributes {
                size: Some(512),
                permissions: Some(0o100600),
                atime: Some(10),
                mtime: Some(20),
                ..FileAttributes::empty()
            },
        };

        assert_packet_roundtrip(Packet::FSetStat(original), |packet| {
            if let Packet::FSetStat(fsetstat) = packet {
                assert_eq!(fsetstat.id, 21);
                assert_eq!(fsetstat.handle.as_ref(), b"opaque-handle");
                assert_eq!(fsetstat.attrs.size, Some(512));
                assert_eq!(fsetstat.attrs.permissions, Some(0o100600));
            } else {
                panic!("Expected FSetStat packet");
            }
        });
    }

    #[test]
    fn packet_extended_roundtrip() {
        let original = Extended {
            id: 31,
            request: "limits@openssh.com".to_string(),
            data: vec![1, 2, 3, 4],
        };

        assert_packet_roundtrip(Packet::Extended(original), |packet| {
            if let Packet::Extended(extended) = packet {
                assert_eq!(extended.id, 31);
                assert_eq!(extended.request, "limits@openssh.com");
                assert_eq!(extended.data, vec![1, 2, 3, 4]);
            } else {
                panic!("Expected Extended packet");
            }
        });
    }

    #[test]
    fn packet_attrs_roundtrip() {
        let original = Attrs {
            id: 41,
            attrs: FileAttributes {
                uid: Some(1000),
                gid: Some(1001),
                permissions: Some(0o040755),
                ..FileAttributes::empty()
            },
        };

        assert_packet_roundtrip(Packet::Attrs(original), |packet| {
            if let Packet::Attrs(attrs) = packet {
                assert_eq!(attrs.id, 41);
                assert_eq!(attrs.attrs.uid, Some(1000));
                assert_eq!(attrs.attrs.gid, Some(1001));
                assert_eq!(attrs.attrs.permissions, Some(0o040755));
            } else {
                panic!("Expected Attrs packet");
            }
        });
    }

    #[test]
    fn packet_handle_preserves_binary_bytes() {
        let mut bytes = BytesMut::new();
        bytes.put_u8(SSH_FXP_HANDLE);
        bytes.put_u32(1);
        bytes.put_u32(2);
        bytes.extend_from_slice(&[0xFF, 0xFE]);

        let mut bytes = bytes.freeze();
        let packet = Packet::try_from(&mut bytes).expect("deserialize binary handle");

        if let Packet::Handle(handle) = packet {
            assert_eq!(handle.handle.as_ref(), &[0xFF, 0xFE]);
        } else {
            panic!("Expected Handle packet");
        }
    }

    #[test]
    fn packet_name_truncated_fails() {
        let mut bytes = BytesMut::new();
        bytes.put_u8(SSH_FXP_NAME);
        bytes.put_u32(1);
        bytes.put_u32(1);
        bytes.put_u32(5);
        bytes.extend_from_slice(b"ab");

        let mut bytes = bytes.freeze();
        let err = Packet::try_from(&mut bytes).expect_err("truncated name should fail");
        assert!(matches!(
            err,
            Error::BadMessage(_) | Error::UnexpectedBehavior(_)
        ));
    }

    #[test]
    fn packet_name_count_too_large_fails() {
        let mut bytes = BytesMut::new();
        bytes.put_u8(SSH_FXP_NAME);
        bytes.put_u32(1);
        bytes.put_u32(2);
        bytes.put_u32(0);
        bytes.put_u32(0);
        bytes.put_u32(0);

        let mut bytes = bytes.freeze();
        let err = Packet::try_from(&mut bytes).expect_err("oversized name count should fail");

        assert!(matches!(err, Error::BadMessage(_)));
    }

    #[test]
    fn serialize_packet_into_preserves_reusable_buffer_capacity() {
        let packet = Packet::Write(Write {
            id: 1,
            handle: Bytes::from_static(b"handle"),
            offset: 0,
            data: Bytes::from(vec![0xAB; 32 * 1024]),
        });
        let mut buf = BytesMut::with_capacity(64 * 1024);
        let initial_capacity = buf.capacity();

        let serialized = serialize_packet_into(packet, &mut buf).expect("serialize packet");

        assert!(!serialized.is_empty());
        assert_eq!(buf.capacity(), initial_capacity);
    }

    #[test]
    fn serialize_packet_into_buf_reuses_existing_allocation() {
        let packet = Packet::Data(Data {
            id: 1,
            data: Bytes::from_static(b"payload").into(),
        });
        let mut buf = BytesMut::with_capacity(64 * 1024);
        let ptr = buf.as_ptr();

        serialize_packet_into_buf(packet, &mut buf).expect("serialize packet");

        assert_eq!(buf.as_ptr(), ptr);
        assert!(!buf.is_empty());
    }

    #[test]
    fn serialize_packet_into_rejects_oversized_packets() {
        let err = checked_packet_length(MAX_PACKET_SIZE + 1)
            .expect_err("oversized packet should be rejected");

        assert!(matches!(err, Error::BadMessage(_)));
    }

    #[test]
    fn serialize_packet_into_data_matches_existing_wire_shape() {
        let packet = Packet::Data(Data {
            id: 7,
            data: Bytes::from_static(b"payload").into(),
        });
        let data = Data {
            id: 7,
            data: Bytes::from_static(b"payload").into(),
        };
        let mut packet_buf = BytesMut::new();

        let packet_bytes =
            serialize_packet_into(packet, &mut packet_buf).expect("serialize packet");
        let payload_bytes = crate::ser::to_bytes(&data).expect("serialize data payload");

        assert_eq!(
            packet_bytes[0..4],
            (payload_bytes.len() as u32 + 1).to_be_bytes()
        );
        assert_eq!(packet_bytes[4], SSH_FXP_DATA);
        assert_eq!(&packet_bytes[5..], &payload_bytes[..]);
    }

    #[test]
    fn serialize_read_packet_matches_generic_wire_shape() {
        let handle = Bytes::from_static(b"handle");
        let packet = Packet::Read(Read {
            id: 9,
            handle: handle.clone(),
            offset: 1234,
            len: 5678,
        });
        let mut packet_buf = BytesMut::new();

        let generic = serialize_packet_into(packet, &mut packet_buf).expect("serialize packet");
        let specialized =
            serialize_read_packet(9, &handle, 1234, 5678).expect("serialize read packet");

        assert_eq!(specialized, generic);
    }

    #[test]
    fn serialize_packet_into_data_handles_empty_payloads() {
        let packet = Packet::Data(Data {
            id: 9,
            data: Bytes::new().into(),
        });
        let mut buf = BytesMut::new();

        let serialized = serialize_packet_into(packet, &mut buf).expect("serialize packet");

        assert_eq!(serialized[4], SSH_FXP_DATA);
        assert_eq!(&serialized[5..9], &9u32.to_be_bytes());
        assert_eq!(&serialized[9..13], &0u32.to_be_bytes());
        assert_eq!(serialized.len(), 13);
    }
}
