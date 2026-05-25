mod handler;

use bytes::Bytes;
use bytes::BytesMut;
use std::fmt;
use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub use self::handler::Handler;

use crate::{
    error::Error,
    protocol::{
        serialize_packet_into_buf, serialize_packet_split, Packet, SerializedPacket, StatusCode,
    },
    utils::read_packet_into_buf,
};

const PACKET_BUF_CAPACITY: usize = 32 * 1024;
const MAX_REUSABLE_WRITE_BUF_SIZE: usize = 256 * 1024;

fn reset_write_buf_if_oversized(write_buf: &mut BytesMut) {
    if write_buf.capacity() > MAX_REUSABLE_WRITE_BUF_SIZE {
        *write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
    } else {
        write_buf.clear();
    }
}

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
    let response = read_response(stream, handler, read_buf).await?;
    let packet = match serialize_packet_split(response, write_buf) {
        Ok(packet) => packet,
        Err(err) => {
            reset_write_buf_if_oversized(write_buf);
            return Err(err);
        }
    };

    if let Err(err) = packet.write_to(stream).await {
        reset_write_buf_if_oversized(write_buf);
        return Err(err.into());
    }
    stream.flush().await?;
    reset_write_buf_if_oversized(write_buf);

    Ok(())
}

async fn process_handler_with_sender<H, S, F, Fut, E>(
    stream: &mut S,
    send_bytes: &mut F,
    handler: &mut H,
    read_buf: &mut BytesMut,
    write_buf: &mut BytesMut,
) -> Result<(), Error>
where
    H: Handler + Send,
    S: AsyncRead + Unpin,
    F: FnMut(Bytes) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: fmt::Display,
{
    let response = read_response(stream, handler, read_buf).await?;

    if let Err(err) = serialize_packet_into_buf(response, write_buf) {
        reset_write_buf_if_oversized(write_buf);
        return Err(err);
    }

    let should_reset_write_buf = write_buf.capacity() > MAX_REUSABLE_WRITE_BUF_SIZE;
    send_bytes(write_buf.split().freeze())
        .await
        .map_err(|err| Error::IO(err.to_string()))?;

    if should_reset_write_buf {
        *write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
    }

    Ok(())
}

async fn process_handler_with_packet_sender<H, S, F, Fut, E>(
    stream: &mut S,
    send_packet: &mut F,
    handler: &mut H,
    read_buf: &mut BytesMut,
    write_buf: &mut BytesMut,
) -> Result<(), Error>
where
    H: Handler + Send,
    S: AsyncRead + Unpin,
    F: FnMut(SerializedPacket) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: fmt::Display,
{
    let response = read_response(stream, handler, read_buf).await?;
    let packet = match serialize_packet_split(response, write_buf) {
        Ok(packet) => packet,
        Err(err) => {
            reset_write_buf_if_oversized(write_buf);
            return Err(err);
        }
    };

    if let Err(err) = send_packet(packet).await {
        reset_write_buf_if_oversized(write_buf);
        return Err(Error::IO(err.to_string()));
    }

    reset_write_buf_if_oversized(write_buf);

    Ok(())
}

async fn read_response<H, S>(
    stream: &mut S,
    handler: &mut H,
    read_buf: &mut BytesMut,
) -> Result<Packet, Error>
where
    H: Handler + Send,
    S: AsyncRead + Unpin,
{
    let mut packet_buf = read_packet_into_buf(stream, read_buf).await?;
    Ok(match Packet::try_from(packet_buf.as_mut_bytes()) {
        Ok(request) => process_request(request, handler).await,
        Err(_) => Packet::error(0, StatusCode::BadMessage),
    })
}

fn keep_serving(result: Result<(), Error>) -> bool {
    match result {
        Err(Error::UnexpectedEof) => false,
        Err(err) => {
            warn!("{}", err);
            false
        }
        Ok(_) => true,
    }
}

/// Run processing stream as SFTP using opaque byte handles and write payloads.
pub async fn run<S, H>(mut stream: S, mut handler: H)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Handler + Send + 'static,
{
    tokio::spawn(async move {
        let mut read_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
        let mut write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);

        while keep_serving(
            process_handler(&mut stream, &mut handler, &mut read_buf, &mut write_buf).await,
        ) {}

        debug!("sftp stream ended");
    });
}

/// Run processing stream as SFTP and send serialized responses as owned bytes.
///
/// This lets integrations with an owned-bytes transport avoid copying the
/// serialized packet through an `AsyncWrite` adapter.
pub async fn run_with_sender<S, H, F, Fut, E>(mut stream: S, mut send_bytes: F, mut handler: H)
where
    S: AsyncRead + Unpin + Send + 'static,
    H: Handler + Send + 'static,
    F: FnMut(Bytes) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send,
    E: fmt::Display + Send,
{
    tokio::spawn(async move {
        serve_with_sender(&mut stream, &mut send_bytes, &mut handler).await;
    });
}

/// Run processing stream as SFTP and send serialized responses as split-aware packets.
pub async fn run_with_packet_sender<S, H, F, Fut, E>(
    mut stream: S,
    mut send_packet: F,
    mut handler: H,
) where
    S: AsyncRead + Unpin + Send + 'static,
    H: Handler + Send + 'static,
    F: FnMut(SerializedPacket) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send,
    E: fmt::Display + Send,
{
    tokio::spawn(async move {
        serve_with_packet_sender(&mut stream, &mut send_packet, &mut handler).await;
    });
}

/// Serve SFTP requests on an existing task and send responses as owned bytes.
///
/// Unlike [`run_with_sender`], this does not spawn and can therefore be used
/// with readers that borrow from an owned transport kept by the caller.
pub async fn serve_with_sender<S, H, F, Fut, E>(stream: &mut S, send_bytes: &mut F, handler: &mut H)
where
    S: AsyncRead + Unpin,
    H: Handler + Send,
    F: FnMut(Bytes) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: fmt::Display,
{
    let mut read_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
    let mut write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);

    while keep_serving(
        process_handler_with_sender(stream, send_bytes, handler, &mut read_buf, &mut write_buf)
            .await,
    ) {}

    debug!("sftp stream ended");
}

/// Serve SFTP requests on an existing task and send split-aware packets.
pub async fn serve_with_packet_sender<S, H, F, Fut, E>(
    stream: &mut S,
    send_packet: &mut F,
    handler: &mut H,
) where
    S: AsyncRead + Unpin,
    H: Handler + Send,
    F: FnMut(SerializedPacket) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: fmt::Display,
{
    let mut read_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);
    let mut write_buf = BytesMut::with_capacity(PACKET_BUF_CAPACITY);

    while keep_serving(
        process_handler_with_packet_sender(
            stream,
            send_packet,
            handler,
            &mut read_buf,
            &mut write_buf,
        )
        .await,
    ) {}

    debug!("sftp stream ended");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{serialize_read_packet, Data, Init, Status, Version, Write};
    use bytes::{Buf, Bytes};
    use std::future::Future;
    use tokio::io::AsyncWriteExt;

    fn ok_status(id: u32) -> Status {
        Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".into(),
            language_tag: "en-US".into(),
        }
    }

    #[derive(Default)]
    struct BytesHandler {
        handle: Option<Bytes>,
        data: Option<Bytes>,
    }

    impl Handler for BytesHandler {
        type Error = StatusCode;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        fn write(
            &mut self,
            id: u32,
            handle: Bytes,
            _offset: u64,
            data: Bytes,
        ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
            self.handle = Some(handle);
            self.data = Some(data);
            async move { Ok(ok_status(id)) }
        }
    }

    struct ReadHandler {
        data: Bytes,
    }

    impl Handler for ReadHandler {
        type Error = StatusCode;

        fn unimplemented(&self) -> Self::Error {
            StatusCode::OpUnsupported
        }

        fn read(
            &mut self,
            id: u32,
            _handle: Bytes,
            _offset: u64,
            _len: u32,
        ) -> impl Future<Output = Result<Data, Self::Error>> + Send {
            let data = self.data.clone();
            async move {
                Ok(Data {
                    id,
                    data: data.into(),
                })
            }
        }
    }

    #[tokio::test]
    async fn run_path_preserves_bytes_handler_api() {
        let mut handler = BytesHandler::default();
        let response = process_request(
            Packet::Write(Write::new(
                8,
                Bytes::from_static(b"bytes-handle"),
                0,
                Bytes::from_static(b"bytes-data"),
            )),
            &mut handler,
        )
        .await;

        assert!(matches!(
            response,
            Packet::Status(Status {
                status_code: StatusCode::Ok,
                ..
            })
        ));
        assert_eq!(
            handler.handle.as_ref().map(Bytes::as_ref),
            Some(&b"bytes-handle"[..])
        );
        assert_eq!(
            handler.data.as_ref().map(Bytes::as_ref),
            Some(&b"bytes-data"[..])
        );
    }

    #[tokio::test]
    async fn run_with_sender_emits_owned_response_bytes() {
        let (mut client, server) = tokio::io::duplex(1024);
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let mut request = BytesMut::new();

        serialize_packet_into_buf(Packet::Init(Init::new()), &mut request).unwrap();

        run_with_sender(
            server,
            move |bytes| {
                let tx = tx.clone();
                async move { tx.send(bytes).await }
            },
            BytesHandler::default(),
        )
        .await;

        client.write_all(&request).await.unwrap();
        let mut response = rx.recv().await.expect("response bytes");
        drop(client);

        response.advance(4);
        assert!(matches!(
            Packet::try_from(&mut response),
            Ok(Packet::Version(Version { .. }))
        ));
    }

    #[tokio::test]
    async fn run_with_packet_sender_emits_split_data_response() {
        let (mut client, server) = tokio::io::duplex(1024);
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let request =
            serialize_read_packet(8, &Bytes::from_static(b"handle"), 0, 7).expect("read packet");

        run_with_packet_sender(
            server,
            move |packet| {
                let tx = tx.clone();
                async move { tx.send(packet).await }
            },
            ReadHandler {
                data: Bytes::from_static(b"payload"),
            },
        )
        .await;

        client.write_all(&request).await.unwrap();
        let packet = rx.recv().await.expect("response packet");
        drop(client);

        match packet {
            SerializedPacket::Split { header, data } => {
                assert_eq!(header.len(), 13);
                assert_eq!(data, Bytes::from_static(b"payload"));

                let mut combined = BytesMut::with_capacity(header.len() + data.len());
                combined.extend_from_slice(&header);
                combined.extend_from_slice(data.as_ref());
                combined.advance(4);

                assert!(matches!(
                    Packet::try_from(&mut combined),
                    Ok(Packet::Data(Data { id: 8, .. }))
                ));
            }
            SerializedPacket::Contiguous(_) => panic!("expected split packet"),
        }
    }
}
