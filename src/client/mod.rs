pub mod error;
pub mod fs;
mod handler;
mod request_session;
mod session;

pub use handler::Handler;
pub use session::SftpSession;

pub type SftpResult<T> = Result<T, error::Error>;

use bytes::{Bytes, BytesMut};
use tokio::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt},
    select,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;

use crate::{error::Error, protocol::Packet, utils::read_packet_into_buf};

macro_rules! into_wrap {
    ($handler:expr) => {
        match $handler.await {
            Err(error) => Err(error.into()),
            Ok(()) => Ok(()),
        }
    };
}

async fn execute_handler(
    bytes: &mut BytesMut,
    handler: &mut impl Handler,
) -> Result<(), error::Error> {
    match Packet::try_from(bytes)? {
        Packet::Version(p) => into_wrap!(handler.version(p)),
        Packet::Status(p) => into_wrap!(handler.status(p)),
        Packet::Handle(p) => into_wrap!(handler.handle(p)),
        Packet::Data(p) => into_wrap!(handler.data(p)),
        Packet::Name(p) => into_wrap!(handler.name(p)),
        Packet::Attrs(p) => into_wrap!(handler.attrs(p)),
        Packet::ExtendedReply(p) => into_wrap!(handler.extended_reply(p)),
        _ => Err(error::Error::UnexpectedBehavior(
            "A packet was received that could not be processed.".to_owned(),
        )),
    }
}

async fn process_handler<S, H>(
    stream: &mut S,
    handler: &mut H,
    read_buf: &mut BytesMut,
) -> Result<(), Error>
where
    S: AsyncRead + Unpin,
    H: Handler + Send,
{
    let mut packet_buf = read_packet_into_buf(stream, read_buf).await?;
    Ok(execute_handler(packet_buf.as_mut_bytes(), handler).await?)
}

/// Run processing stream as SFTP client. Is a simple handler of incoming
/// and outgoing packets. Can be used for non-standard implementations
pub fn run<S, H>(stream: S, mut handler: H) -> mpsc::UnboundedSender<Bytes>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    H: Handler + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<Bytes>();
    let (mut rd, mut wr) = io::split(stream);

    let rc = CancellationToken::new();
    let wc = rc.clone();
    {
        tokio::spawn(async move {
            let mut read_buf = BytesMut::with_capacity(32 * 1024);
            loop {
                select! {
                    result = process_handler(&mut rd, &mut handler, &mut read_buf) => {
                        match result {
                            Err(Error::UnexpectedEof) => break,
                            Err(err) => warn!("{}", err),
                            Ok(_) => (),
                        }
                    },
                    _ = rc.cancelled() => break,
                }
            }

            rc.cancel();
            debug!("read half of sftp stream ended");
        });
    }

    tokio::spawn(async move {
        loop {
            select! {
                Some(data) = rx.recv() => {
                    if data.is_empty() {
                        let _ = wr.shutdown().await;
                        break;
                    }

                    let _ = wr.write_all(&data[..]).await;
                },
                _ = wc.cancelled() => break,
            }
        }

        wc.cancel();
        debug!("write half of sftp stream ended");
    });

    tx
}
