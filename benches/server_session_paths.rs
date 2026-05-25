use criterion::{criterion_group, criterion_main, Criterion};
use russh_sftp::{
    protocol::{Data, FileAttributes, Handle, Open, OpenFlags, Packet, Read, Status, StatusCode},
    server::{self, Handler, ManagedSession, SessionHandler},
};
use std::{future::Future, hint::black_box};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const OPEN_ID: u32 = 1;
const READ_ID: u32 = 2;
const CLOSE_ID: u32 = 3;

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".into(),
        language_tag: "en".into(),
    }
}

struct RawHandler;

impl Handler for RawHandler {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    fn open(
        &mut self,
        id: u32,
        filename: String,
        _pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Handle, Self::Error>> + Send {
        async move {
            Ok(Handle {
                id,
                handle: filename,
            })
        }
    }

    fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> impl Future<Output = Result<Data, Self::Error>> + Send {
        async move {
            Ok(Data {
                id,
                data: format!("{handle}:{offset}:{len}").into_bytes(),
            })
        }
    }

    fn close(
        &mut self,
        id: u32,
        _handle: String,
    ) -> impl Future<Output = Result<Status, Self::Error>> + Send {
        async move { Ok(ok_status(id)) }
    }
}

struct TypedHandler;

impl SessionHandler for TypedHandler {
    type Error = StatusCode;
    type File = String;
    type Dir = ();

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    fn open(
        &mut self,
        _id: u32,
        filename: String,
        _pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> impl Future<Output = Result<Self::File, Self::Error>> + Send {
        async move { Ok(filename) }
    }

    fn read<'a>(
        &'a mut self,
        id: u32,
        file: &'a mut Self::File,
        offset: u64,
        len: u32,
    ) -> impl Future<Output = Result<Data, Self::Error>> + Send + 'a {
        async move {
            Ok(Data {
                id,
                data: format!("{file}:{offset}:{len}").into_bytes(),
            })
        }
    }
}

async fn send_packet<W: AsyncWrite + Unpin>(stream: &mut W, packet: Packet) {
    let bytes = Vec::<u8>::from(bytes::Bytes::try_from(packet).expect("serialize request"));
    stream.write_all(&bytes).await.expect("write request");
}

async fn read_response<W: AsyncRead + Unpin>(stream: &mut W) -> Packet {
    let length = stream.read_u32().await.expect("read response length");
    let mut bytes = bytes::BytesMut::zeroed(length as usize);
    stream
        .read_exact(&mut bytes)
        .await
        .expect("read response payload");
    let mut bytes = bytes.freeze();
    Packet::try_from(&mut bytes).expect("parse response")
}

async fn run_open_read_close<H: Handler + Send + 'static>(handler: H) {
    let (mut client, server) = tokio::io::duplex(4096);
    server::run(server, handler).await;

    send_packet(
        &mut client,
        Packet::Open(Open {
            id: OPEN_ID,
            filename: "bench-file".into(),
            pflags: OpenFlags::READ,
            attrs: FileAttributes::empty(),
        }),
    )
    .await;
    let handle = match read_response(&mut client).await {
        Packet::Handle(handle) => handle.handle,
        packet => panic!("unexpected open response: {packet:?}"),
    };

    send_packet(
        &mut client,
        Packet::Read(Read {
            id: READ_ID,
            handle: handle.clone(),
            offset: 7,
            len: 9,
        }),
    )
    .await;
    black_box(read_response(&mut client).await);

    send_packet(
        &mut client,
        Packet::Close(russh_sftp::protocol::Close {
            id: CLOSE_ID,
            handle,
        }),
    )
    .await;
    black_box(read_response(&mut client).await);
}

fn bench_server_paths(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut group = c.benchmark_group("server_session_paths");

    group.bench_function("raw_handler", |b| {
        b.to_async(&runtime)
            .iter(|| run_open_read_close(RawHandler));
    });

    group.bench_function("managed_session", |b| {
        b.to_async(&runtime)
            .iter(|| run_open_read_close(ManagedSession::new(TypedHandler)));
    });

    group.finish();
}

criterion_group!(benches, bench_server_paths);
criterion_main!(benches);
