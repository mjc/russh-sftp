use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::Error;

pub fn unix(time: SystemTime) -> u32 {
    DateTime::<Utc>::from(time).timestamp() as u32
}

pub async fn read_packet<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Bytes, Error> {
    let length = stream.read_u32().await?;

    let mut buf = BytesMut::zeroed(length as usize);
    
    stream.read_exact(&mut buf).await?;

    Ok(buf.freeze())
}
