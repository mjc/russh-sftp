use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::Error;

pub(crate) const MAX_PACKET_SIZE: usize = 256 * 1024 * 1024;
const MAX_REUSABLE_PACKET_SIZE: usize = 256 * 1024;

pub fn unix(time: SystemTime) -> u32 {
    DateTime::<Utc>::from(time).timestamp() as u32
}

#[derive(Debug)]
pub(crate) enum PacketBuffer<'a> {
    Reusable(&'a mut BytesMut),
    Owned(BytesMut),
}

impl PacketBuffer<'_> {
    pub(crate) fn as_mut_bytes(&mut self) -> &mut BytesMut {
        match self {
            Self::Reusable(buf) => buf,
            Self::Owned(buf) => buf,
        }
    }
}

fn reusable_capacity_for(length: usize) -> usize {
    if length == 0 {
        return 0;
    }

    length
        .checked_next_power_of_two()
        .unwrap_or(MAX_REUSABLE_PACKET_SIZE)
        .min(MAX_REUSABLE_PACKET_SIZE)
}

async fn read_packet_payload<S: AsyncRead + Unpin>(
    stream: &mut S,
    target: &mut BytesMut,
    length: usize,
) -> Result<(), Error> {
    target.clear();

    let desired_capacity = reusable_capacity_for(length).max(length);
    if desired_capacity > target.capacity() {
        target.reserve(desired_capacity);
    }

    while target.len() < length {
        let remaining = length - target.len();
        let read = stream.take(remaining as u64).read_buf(target).await?;
        if read == 0 {
            return Err(Error::UnexpectedEof);
        }
    }

    Ok(())
}

pub(crate) async fn read_packet_into_buf<'a, S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &'a mut BytesMut,
) -> Result<PacketBuffer<'a>, Error> {
    let length = stream.read_u32().await? as usize;
    if length > MAX_PACKET_SIZE {
        return Err(Error::BadMessage(format!(
            "packet length {} exceeds maximum {}",
            length, MAX_PACKET_SIZE
        )));
    }

    if length <= MAX_REUSABLE_PACKET_SIZE {
        read_packet_payload(stream, buf, length).await?;
        return Ok(PacketBuffer::Reusable(buf));
    }

    let mut packet = BytesMut::with_capacity(length);
    read_packet_payload(stream, &mut packet, length).await?;
    Ok(PacketBuffer::Owned(packet))
}

/// Read a packet into the provided buffer, returning owned `Bytes`.
/// Small packets are read through the reusable buffer and returned by shallowly
/// cloning and freezing that buffer, so the returned `Bytes` may share the
/// allocation until a later copy-on-write mutation. Oversized packets return a
/// directly frozen owned buffer. Call [`read_packet_into_buf`] for the internal
/// zero-copy buffer API.
#[allow(dead_code)]
pub async fn read_packet_into<S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &mut BytesMut,
) -> Result<Bytes, Error> {
    match read_packet_into_buf(stream, buf).await? {
        PacketBuffer::Reusable(buf) => Ok(buf.clone().freeze()),
        PacketBuffer::Owned(buf) => Ok(buf.freeze()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    struct ChunkedReader {
        chunks: Vec<Vec<u8>>,
        chunk_idx: usize,
        offset: usize,
    }

    impl ChunkedReader {
        fn new(chunks: Vec<Vec<u8>>) -> Self {
            Self {
                chunks,
                chunk_idx: 0,
                offset: 0,
            }
        }
    }

    impl AsyncRead for ChunkedReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            while self.chunk_idx < self.chunks.len() {
                let chunk = &self.chunks[self.chunk_idx];
                if self.offset == chunk.len() {
                    self.chunk_idx += 1;
                    self.offset = 0;
                    continue;
                }

                let remaining = &chunk[self.offset..];
                let to_copy = remaining.len().min(buf.remaining());
                buf.put_slice(&remaining[..to_copy]);
                self.offset += to_copy;
                return Poll::Ready(Ok(()));
            }

            Poll::Ready(Ok(()))
        }
    }

    fn packet_chunks(payload_chunks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let total_len = payload_chunks.iter().map(Vec::len).sum::<usize>() as u32;
        let mut chunks = vec![total_len.to_be_bytes().to_vec()];
        chunks.extend(payload_chunks);
        chunks
    }

    async fn read_packet(chunks: Vec<Vec<u8>>, capacity: usize) -> Result<Bytes, Error> {
        let mut reader = ChunkedReader::new(packet_chunks(chunks));
        let mut buf = BytesMut::with_capacity(capacity);
        read_packet_into(&mut reader, &mut buf).await
    }

    fn assert_reusable(buffer: PacketBuffer<'_>, expected: &[u8]) {
        match buffer {
            PacketBuffer::Reusable(reusable) => assert_eq!(reusable.as_ref(), expected),
            PacketBuffer::Owned(_) => panic!("expected reusable packet buffer"),
        }
    }

    #[tokio::test]
    async fn read_packet_into_reads_empty_single_byte_and_partial_packets() {
        for (chunks, capacity, expected) in [
            (vec![vec![]], 16, &b""[..]),
            (vec![vec![0xAB]], 16, &[0xAB][..]),
            (
                vec![b"he".to_vec(), b"ll".to_vec(), b"o".to_vec()],
                2,
                &b"hello"[..],
            ),
        ] {
            let packet = read_packet(chunks, capacity).await.expect("read packet");
            assert_eq!(packet.as_ref(), expected);
        }
    }

    #[tokio::test]
    async fn read_packet_into_returns_unexpected_eof_for_short_packet() {
        let mut reader = ChunkedReader::new(vec![5u32.to_be_bytes().to_vec(), b"abc".to_vec()]);
        let mut buf = BytesMut::with_capacity(16);

        let err = read_packet_into(&mut reader, &mut buf)
            .await
            .expect_err("short packet should fail");

        assert!(matches!(err, Error::UnexpectedEof));
    }

    #[tokio::test]
    async fn read_packet_into_does_not_grow_reusable_buffer_for_large_packets() {
        let payload = vec![0xCD; MAX_REUSABLE_PACKET_SIZE + 1];
        let mut reader = ChunkedReader::new(packet_chunks(vec![payload.clone()]));
        let mut buf = BytesMut::with_capacity(32 * 1024);
        let initial_capacity = buf.capacity();

        let packet = read_packet_into(&mut reader, &mut buf)
            .await
            .expect("read packet");

        assert_eq!(packet.len(), payload.len());
        assert_eq!(packet.as_ref(), payload.as_slice());
        assert_eq!(buf.capacity(), initial_capacity);
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn read_packet_into_rejects_oversized_packets() {
        let too_large = (MAX_PACKET_SIZE as u32).wrapping_add(1);
        let mut reader = ChunkedReader::new(vec![too_large.to_be_bytes().to_vec()]);
        let mut buf = BytesMut::with_capacity(16);

        let err = read_packet_into(&mut reader, &mut buf)
            .await
            .expect_err("packet should be rejected");

        assert!(matches!(err, Error::BadMessage(_)));
    }

    #[tokio::test]
    async fn read_packet_into_preserves_reusable_buffer_capacity() {
        let payload = vec![0xAB; 32 * 1024];
        let mut reader = ChunkedReader::new(packet_chunks(vec![payload.clone()]));
        let mut buf = BytesMut::with_capacity(64 * 1024);
        let initial_capacity = buf.capacity();

        let packet = read_packet_into(&mut reader, &mut buf)
            .await
            .expect("read packet");

        assert_eq!(packet.as_ref(), payload.as_slice());
        assert_eq!(buf.capacity(), initial_capacity);
        assert_eq!(buf.len(), payload.len());
    }

    #[tokio::test]
    async fn read_packet_into_grows_reusable_buffer_once_then_reuses_capacity() {
        let first = vec![0xAB; 33 * 1024];
        let second = vec![0xCD; 34 * 1024];
        let mut reader = ChunkedReader::new(vec![
            (first.len() as u32).to_be_bytes().to_vec(),
            first.clone(),
            (second.len() as u32).to_be_bytes().to_vec(),
            second.clone(),
        ]);
        let mut buf = BytesMut::with_capacity(32 * 1024);

        let first_packet = read_packet_into(&mut reader, &mut buf)
            .await
            .expect("first packet");
        let grown_capacity = buf.capacity();

        let second_packet = read_packet_into(&mut reader, &mut buf)
            .await
            .expect("second packet");

        assert_eq!(first_packet.as_ref(), first.as_slice());
        assert_eq!(second_packet.as_ref(), second.as_slice());
        assert!(grown_capacity >= 64 * 1024);
        assert!(grown_capacity <= MAX_REUSABLE_PACKET_SIZE);
        assert_eq!(buf.capacity(), grown_capacity);
    }

    #[tokio::test]
    async fn read_packet_into_buf_returns_reusable_or_owned_by_size() {
        let mut reader = ChunkedReader::new(packet_chunks(vec![b"ping".to_vec()]));
        let mut buf = BytesMut::with_capacity(16);

        assert_reusable(
            read_packet_into_buf(&mut reader, &mut buf)
                .await
                .expect("read small packet"),
            b"ping",
        );

        let payload = vec![0xEF; MAX_REUSABLE_PACKET_SIZE + 1];
        let mut reader = ChunkedReader::new(packet_chunks(vec![payload.clone()]));

        let result = read_packet_into_buf(&mut reader, &mut buf)
            .await
            .expect("read large packet");

        if let PacketBuffer::Owned(owned) = result {
            assert_eq!(owned.freeze().as_ref(), payload.as_slice());
        } else {
            panic!("Expected owned packet buffer");
        }
    }

    #[tokio::test]
    async fn read_packet_into_buf_rejects_oversized_packets() {
        let too_large = (MAX_PACKET_SIZE as u32).wrapping_add(1);
        let mut reader = ChunkedReader::new(vec![too_large.to_be_bytes().to_vec()]);
        let mut buf = BytesMut::with_capacity(16);

        let err = read_packet_into_buf(&mut reader, &mut buf)
            .await
            .expect_err("oversized packet should be rejected");

        assert!(matches!(err, Error::BadMessage(_)));
    }
}
