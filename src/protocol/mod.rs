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

use crate::{de, error::Error, ser};

pub use self::{
    attrs::Attrs,
    close::Close,
    data::Data,
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

    pub fn status(id: u32, status_code: StatusCode, msg: &str, tag: &str) -> Self {
        Packet::Status(Status {
            id,
            status_code,
            error_message: msg.to_string(),
            language_tag: tag.to_string(),
        })
    }

    pub fn error(id: u32, status_code: StatusCode) -> Self {
        Self::status(id, status_code, &status_code.to_string(), "en-US")
    }
}

impl TryFrom<&mut Bytes> for Packet {
    type Error = Error;

    fn try_from(bytes: &mut Bytes) -> Result<Self, Self::Error> {
        let r#type = bytes.try_get_u8()?;
        debug!("packet type {}", r#type);

        let request = match r#type {
            SSH_FXP_INIT => Self::Init(de::from_bytes(bytes)?),
            SSH_FXP_VERSION => Self::Version(de::from_bytes(bytes)?),
            SSH_FXP_OPEN => Self::Open(de::from_bytes(bytes)?),
            SSH_FXP_CLOSE => Self::Close(Close::from_bytes(bytes)?),
            // Manual deserialization for consistency with Write/Data
            SSH_FXP_READ => Self::Read(Read::from_bytes(bytes)?),
            // Zero-copy deserialization - bypasses serde to avoid Vec allocation
            SSH_FXP_WRITE => Self::Write(Write::from_bytes(bytes)?),
            SSH_FXP_LSTAT => Self::Lstat(de::from_bytes(bytes)?),
            SSH_FXP_FSTAT => Self::Fstat(Fstat::from_bytes(bytes)?),
            SSH_FXP_SETSTAT => Self::SetStat(de::from_bytes(bytes)?),
            SSH_FXP_FSETSTAT => Self::FSetStat(de::from_bytes(bytes)?),
            SSH_FXP_OPENDIR => Self::OpenDir(de::from_bytes(bytes)?),
            SSH_FXP_READDIR => Self::ReadDir(ReadDir::from_bytes(bytes)?),
            SSH_FXP_REMOVE => Self::Remove(de::from_bytes(bytes)?),
            SSH_FXP_MKDIR => Self::MkDir(de::from_bytes(bytes)?),
            SSH_FXP_RMDIR => Self::RmDir(de::from_bytes(bytes)?),
            SSH_FXP_REALPATH => Self::RealPath(de::from_bytes(bytes)?),
            SSH_FXP_STAT => Self::Stat(de::from_bytes(bytes)?),
            SSH_FXP_RENAME => Self::Rename(de::from_bytes(bytes)?),
            SSH_FXP_READLINK => Self::ReadLink(de::from_bytes(bytes)?),
            SSH_FXP_SYMLINK => Self::Symlink(de::from_bytes(bytes)?),
            SSH_FXP_STATUS => Self::Status(de::from_bytes(bytes)?),
            SSH_FXP_HANDLE => Self::Handle(de::from_bytes(bytes)?),
            // Zero-copy deserialization - bypasses serde to avoid Vec allocation
            SSH_FXP_DATA => Self::Data(Data::from_bytes(bytes)?),
            SSH_FXP_NAME => Self::Name(de::from_bytes(bytes)?),
            SSH_FXP_ATTRS => Self::Attrs(de::from_bytes(bytes)?),
            SSH_FXP_EXTENDED => Self::Extended(de::from_bytes(bytes)?),
            SSH_FXP_EXTENDED_REPLY => Self::ExtendedReply(de::from_bytes(bytes)?),
            _ => return Err(Error::BadMessage("unknown type".to_owned())),
        };

        Ok(request)
    }
}

macro_rules! serialize_packet {
    ($bytes:expr, $($variant:ident => $type_const:expr),+ $(,)?) => {
        |packet: Packet, bytes: &mut BytesMut| -> Result<(), Error> {
            match packet {
                $(
                    Packet::$variant(v) => {
                        bytes.put_u8($type_const);
                        ser::to_bytes_into(&v, bytes)?;
                    }
                )+
            }
            Ok(())
        }
    };
}

/// A serialized packet that may consist of a header and a separate data payload.
/// This enables zero-copy serialization for Data packets: the payload bytes are
/// kept as-is rather than being copied into the header buffer.
pub enum SerializedPacket {
    /// A single contiguous packet (most packet types).
    Contiguous(Bytes),
    /// A header + separate data payload (Data packets on the download path).
    /// The header contains [length:4][type:1][id:4][data_len:4] and the
    /// payload is the original Bytes without copying.
    Split { header: Bytes, data: Bytes },
}

impl SerializedPacket {
    /// Total wire length (for diagnostics / testing).
    pub fn wire_len(&self) -> usize {
        match self {
            Self::Contiguous(b) => b.len(),
            Self::Split { header, data } => header.len() + data.len(),
        }
    }

    /// Write to an AsyncWrite stream. Uses two writes for Split packets
    /// to avoid copying the data payload into the header buffer.
    pub async fn write_to<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
    ) -> Result<(), std::io::Error> {
        use tokio::io::AsyncWriteExt;
        match self {
            Self::Contiguous(b) => {
                stream.write_all(b).await?;
            }
            Self::Split { header, data } => {
                stream.write_all(header).await?;
                stream.write_all(data).await?;
            }
        }
        Ok(())
    }
}

/// Serialize a packet, splitting Data packets to avoid copying the payload.
/// For Data packets, returns `Split { header, data }` where the data bytes
/// are passed through without copying. For all other packets, returns
/// `Contiguous(bytes)` using the provided buffer.
pub fn serialize_packet_split(packet: Packet, buf: &mut BytesMut) -> Result<SerializedPacket, Error> {
    // Fast path: Data packets get zero-copy treatment
    if let Packet::Data(data) = packet {
        // Header layout: [total_len:4][SSH_FXP_DATA:1][id:4][data_len:4] = 13 bytes
        // total_len = 1 + 4 + 4 + data.data.len() (excludes the 4-byte length field)
        let total_len = (1 + 4 + 4 + data.data.len()) as u32;
        buf.clear();
        buf.reserve(13);
        buf.put_u32(total_len);
        buf.put_u8(SSH_FXP_DATA);
        buf.put_u32(data.id);
        buf.put_u32(data.data.len() as u32);
        return Ok(SerializedPacket::Split {
            header: buf.split().freeze(),
            data: data.data,
        });
    }

    // Normal path: serialize into contiguous buffer
    let bytes = serialize_packet_into(packet, buf)?;
    Ok(SerializedPacket::Contiguous(bytes))
}

/// Serialize a packet into an existing buffer, returning the bytes.
/// Clears the buffer first but reuses its capacity to avoid allocation.
pub fn serialize_packet_into(packet: Packet, buf: &mut BytesMut) -> Result<Bytes, Error> {
    buf.clear();

    // Estimate capacity based on packet type to avoid reallocations
    let capacity = match &packet {
        Packet::Write(w) => 32 + w.handle.len() + w.data.len(),
        Packet::Data(d) => 16 + d.data.len(),
        _ => 256,
    };
    buf.reserve(capacity);

    // Single buffer: [length:4][type:1][payload...]
    buf.put_u32(0); // placeholder for length

    let serializer = serialize_packet!(buf,
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
        Status => SSH_FXP_STATUS,
        Handle => SSH_FXP_HANDLE,
        Data => SSH_FXP_DATA,
        Name => SSH_FXP_NAME,
        Attrs => SSH_FXP_ATTRS,
        Extended => SSH_FXP_EXTENDED,
        ExtendedReply => SSH_FXP_EXTENDED_REPLY,
    );
    serializer(packet, buf)?;

    // Patch length (excludes the 4-byte length field itself)
    let length = (buf.len() - 4) as u32;
    buf[0..4].copy_from_slice(&length.to_be_bytes());

    Ok(buf.split().freeze())
}

impl TryFrom<Packet> for Bytes {
    type Error = Error;

    fn try_from(packet: Packet) -> Result<Self, Self::Error> {
        let mut buf = BytesMut::new();
        serialize_packet_into(packet, &mut buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_write_roundtrip() {
        let original = Write {
            id: 42,
            handle: Bytes::from_static(b"test-handle"),
            offset: 1024,
            data: Bytes::from_static(b"hello world"),
        };

        // Serialize to packet bytes
        let serialized: Bytes = Packet::Write(original).try_into().expect("serialize failed");

        // Skip the 4-byte length prefix and parse
        let mut bytes = serialized.slice(4..);
        let packet = Packet::try_from(&mut bytes).expect("deserialize failed");

        if let Packet::Write(write) = packet {
            assert_eq!(write.id, 42);
            assert_eq!(write.handle.as_ref(), b"test-handle");
            assert_eq!(write.offset, 1024);
            assert_eq!(write.data.as_ref(), b"hello world");
        } else {
            panic!("Expected Write packet");
        }
    }

    #[test]
    fn packet_data_roundtrip() {
        let original = Data {
            id: 99,
            data: Bytes::from(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };

        let serialized: Bytes = Packet::Data(original).try_into().expect("serialize failed");

        let mut bytes = serialized.slice(4..);
        let packet = Packet::try_from(&mut bytes).expect("deserialize failed");

        if let Packet::Data(data) = packet {
            assert_eq!(data.id, 99);
            assert_eq!(data.data.as_ref(), &[0xDE, 0xAD, 0xBE, 0xEF]);
        } else {
            panic!("Expected Data packet");
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

        let serialized: Bytes = Packet::Write(original).try_into().expect("serialize failed");

        let mut bytes = serialized.slice(4..);
        let packet = Packet::try_from(&mut bytes).expect("deserialize failed");

        if let Packet::Write(write) = packet {
            assert_eq!(write.data.len(), 64 * 1024);
            assert_eq!(write.data.as_ref(), large_data.as_slice());
        } else {
            panic!("Expected Write packet");
        }
    }

    #[test]
    fn serialize_packet_split_data_matches_contiguous() {
        // Verify that split serialization produces identical wire bytes
        let payload = vec![0xCAu8; 32 * 1024]; // 32KB payload

        // Contiguous path
        let mut buf1 = BytesMut::new();
        let contiguous = serialize_packet_into(
            Packet::Data(Data { id: 42, data: Bytes::from(payload.clone()) }),
            &mut buf1,
        )
        .expect("contiguous serialize failed");

        // Split path
        let mut buf2 = BytesMut::new();
        let split = serialize_packet_split(
            Packet::Data(Data { id: 42, data: Bytes::from(payload) }),
            &mut buf2,
        )
        .expect("split serialize failed");

        // Reassemble the split version
        let reassembled = match split {
            SerializedPacket::Split { header, data } => {
                let mut combined = BytesMut::with_capacity(header.len() + data.len());
                combined.extend_from_slice(&header);
                combined.extend_from_slice(&data);
                combined.freeze()
            }
            SerializedPacket::Contiguous(b) => b,
        };

        assert_eq!(contiguous, reassembled, "split and contiguous must produce identical wire bytes");
    }

    #[test]
    fn serialize_packet_split_non_data_is_contiguous() {
        // Non-Data packets should always produce Contiguous
        let original = Write {
            id: 7,
            handle: Bytes::from_static(b"handle"),
            offset: 512,
            data: Bytes::from_static(b"payload"),
        };

        let mut buf = BytesMut::new();
        let result = serialize_packet_split(
            Packet::Write(original),
            &mut buf,
        )
        .expect("split serialize failed");

        assert!(
            matches!(result, SerializedPacket::Contiguous(_)),
            "non-Data packets should produce Contiguous"
        );
    }
}
