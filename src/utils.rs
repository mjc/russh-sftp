use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::Error;

pub fn unix(time: SystemTime) -> u32 {
    DateTime::<Utc>::from(time).timestamp() as u32
}

/// Read a packet into the provided buffer, returning a Bytes view of the data.
/// The buffer is reused across calls - cleared but not zeroed.
pub async fn read_packet_into<S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &mut BytesMut,
) -> Result<Bytes, Error> {
    use tokio::io::AsyncReadExt;

    let length = stream.read_u32().await? as usize;

    buf.clear();

    // Limit the reader to exactly `length` bytes, then read_buf into spare capacity.
    // This avoids zeroing while preventing over-reading.
    let mut limited = stream.take(length as u64);
    buf.reserve(length);

    while buf.len() < length {
        let n = limited.read_buf(buf).await?;
        if n == 0 {
            return Err(Error::UnexpectedEof);
        }
    }

    Ok(buf.split().freeze())
}

pub async fn read_packet<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Bytes, Error> {
    let length = stream.read_u32().await?;

    let mut buf = BytesMut::zeroed(length as usize);

    stream.read_exact(&mut buf).await?;

    Ok(buf.freeze())
}
