mod handler;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub use self::handler::Handler;

use bytes::BytesMut;

use crate::{
    error::Error,
    protocol::{serialize_packet_split, Packet, StatusCode},
    utils::read_packet_into,
};

macro_rules! into_wrap {
    ($id:expr, $handler:expr, $var:ident; $($arg:ident),*) => {
        match $handler.$var($($var.$arg),*).await {
            Err(err) => Packet::error($id, err.into()),
            Ok(packet) => packet.into(),
        }
    };
}

async fn process_request<H>(packet: Packet, handler: &mut H) -> Packet
where
    H: Handler + Send,
{
    let id = packet.get_request_id();

    match packet {
        Packet::Init(init) => into_wrap!(id, handler, init; version, extensions),
        Packet::Open(open) => into_wrap!(id, handler, open; id, filename, pflags, attrs),
        Packet::Close(close) => into_wrap!(id, handler, close; id, handle),
        Packet::Read(read) => into_wrap!(id, handler, read; id, handle, offset, len),
        Packet::Write(write) => into_wrap!(id, handler, write; id, handle, offset, data),
        Packet::Lstat(lstat) => into_wrap!(id, handler, lstat; id, path),
        Packet::Fstat(fstat) => into_wrap!(id, handler, fstat; id, handle),
        Packet::SetStat(setstat) => into_wrap!(id, handler, setstat; id, path, attrs),
        Packet::FSetStat(fsetstat) => into_wrap!(id, handler, fsetstat; id, handle, attrs),
        Packet::OpenDir(opendir) => into_wrap!(id, handler, opendir; id, path),
        Packet::ReadDir(readdir) => into_wrap!(id, handler, readdir; id, handle),
        Packet::Remove(remove) => into_wrap!(id, handler, remove; id, filename),
        Packet::MkDir(mkdir) => into_wrap!(id, handler, mkdir; id, path, attrs),
        Packet::RmDir(rmdir) => into_wrap!(id, handler, rmdir; id, path),
        Packet::RealPath(realpath) => into_wrap!(id, handler, realpath; id, path),
        Packet::Stat(stat) => into_wrap!(id, handler, stat; id, path),
        Packet::Rename(rename) => into_wrap!(id, handler, rename; id, oldpath, newpath),
        Packet::ReadLink(readlink) => into_wrap!(id, handler, readlink; id, path),
        Packet::Symlink(symlink) => into_wrap!(id, handler, symlink; id, linkpath, targetpath),
        Packet::Extended(extended) => into_wrap!(id, handler, extended; id, request, data),
        _ => Packet::error(0, StatusCode::BadMessage),
    }
}

async fn process_handler<H, S>(
    stream: &mut S,
    handler: &mut H,
    read_buf: &mut BytesMut,
    write_buf: &mut BytesMut,
) -> Result<(), Error>
where
    H: Handler + Send,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut bytes = read_packet_into(stream, read_buf).await?;

    let response = match Packet::try_from(&mut bytes) {
        Ok(request) => process_request(request, handler).await,
        Err(_) => Packet::error(0, StatusCode::BadMessage),
    };

    let packet = serialize_packet_split(response, write_buf)?;
    packet.write_to(stream).await?;
    stream.flush().await?;

    Ok(())
}

/// Run processing stream as SFTP
pub async fn run<S, H>(mut stream: S, mut handler: H)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Handler + Send + 'static,
{
    tokio::spawn(async move {
        // Reusable buffers for reading/writing packets - avoids allocation per packet
        let mut read_buf = BytesMut::with_capacity(32 * 1024);
        let mut write_buf = BytesMut::with_capacity(32 * 1024);

        loop {
            match process_handler(&mut stream, &mut handler, &mut read_buf, &mut write_buf).await {
                Err(Error::UnexpectedEof) => break,
                Err(err) => warn!("{}", err),
                Ok(_) => (),
            }
        }

        debug!("sftp stream ended");
    });
}
