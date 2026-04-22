use bytes::Bytes;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use russh_sftp::{
    protocol::{
        Data, File, FileAttributes, Handle, Name, Open, OpenFlags, Packet, Status, StatusCode,
        Write,
    },
    ser,
};
use std::hint::black_box;

fn write_fixture(size: usize) -> Write {
    Write {
        id: 42,
        handle: Bytes::from_static(b"test-handle"),
        offset: 1024,
        data: Bytes::from(vec![0xAB; size]),
    }
}

fn data_fixture(size: usize) -> Data {
    Data {
        id: 43,
        data: Bytes::from(vec![0xCD; size]),
    }
}

fn handle_fixture(size: usize) -> Handle {
    Handle {
        id: 44,
        handle: Bytes::from("h".repeat(size)),
    }
}

fn status_fixture(size: usize) -> Status {
    Status {
        id: 45,
        status_code: StatusCode::Failure,
        error_message: "e".repeat(size),
        language_tag: "en-US".to_string(),
    }
}

fn name_fixture(entries: usize) -> Name {
    Name {
        id: 46,
        files: (0..entries)
            .map(|i| File {
                filename: format!("file-{i}"),
                longname: format!("-rw-r--r-- file-{i}"),
                attrs: FileAttributes {
                    size: Some(i as u64),
                    permissions: Some(0o100644),
                    ..FileAttributes::empty()
                },
            })
            .collect(),
    }
}

fn open_fixture(size: usize) -> Open {
    Open {
        id: 47,
        filename: format!("/{}", "p".repeat(size)),
        pflags: OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE,
        attrs: FileAttributes {
            size: Some(size as u64),
            permissions: Some(0o100644),
            ..FileAttributes::empty()
        },
    }
}

fn serialized_write_fixture(size: usize) -> Bytes {
    ser::to_bytes(&write_fixture(size)).expect("serialize write fixture")
}

fn serialized_data_fixture(size: usize) -> Bytes {
    ser::to_bytes(&data_fixture(size)).expect("serialize data fixture")
}

fn serialized_handle_fixture(size: usize) -> Bytes {
    ser::to_bytes(&handle_fixture(size)).expect("serialize handle fixture")
}

fn serialized_status_fixture(size: usize) -> Bytes {
    ser::to_bytes(&status_fixture(size)).expect("serialize status fixture")
}

fn serialized_name_fixture(entries: usize) -> Bytes {
    ser::to_bytes(&name_fixture(entries)).expect("serialize name fixture")
}

fn serialized_open_fixture(size: usize) -> Bytes {
    ser::to_bytes(&open_fixture(size)).expect("serialize open fixture")
}

fn write_packet_fixture(size: usize) -> Packet {
    Packet::Write(write_fixture(size))
}

fn data_packet_fixture(size: usize) -> Packet {
    Packet::Data(data_fixture(size))
}

fn serialized_write_packet_fixture(size: usize) -> Bytes {
    let packet: Bytes = write_packet_fixture(size)
        .try_into()
        .expect("serialize write packet fixture");
    packet.slice(4..)
}

fn serialized_data_packet_fixture(size: usize) -> Bytes {
    let packet: Bytes = data_packet_fixture(size)
        .try_into()
        .expect("serialize data packet fixture");
    packet.slice(4..)
}

#[library_benchmark]
#[bench::size_1k(write_fixture(1024))]
#[bench::size_4k(write_fixture(4 * 1024))]
#[bench::size_16k(write_fixture(16 * 1024))]
#[bench::size_64k(write_fixture(64 * 1024))]
fn serialize_write(packet: Write) {
    black_box(ser::to_bytes(&packet).expect("serialize write"));
}

#[library_benchmark]
#[bench::size_1k(serialized_write_fixture(1024))]
#[bench::size_4k(serialized_write_fixture(4 * 1024))]
#[bench::size_16k(serialized_write_fixture(16 * 1024))]
#[bench::size_64k(serialized_write_fixture(64 * 1024))]
fn deserialize_write(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Write::from_bytes(&mut bytes).expect("deserialize write"));
}

#[library_benchmark]
#[bench::size_1k(data_fixture(1024))]
#[bench::size_4k(data_fixture(4 * 1024))]
#[bench::size_16k(data_fixture(16 * 1024))]
#[bench::size_64k(data_fixture(64 * 1024))]
fn serialize_data(packet: Data) {
    black_box(ser::to_bytes(&packet).expect("serialize data"));
}

#[library_benchmark]
#[bench::size_1k(serialized_data_fixture(1024))]
#[bench::size_4k(serialized_data_fixture(4 * 1024))]
#[bench::size_16k(serialized_data_fixture(16 * 1024))]
#[bench::size_64k(serialized_data_fixture(64 * 1024))]
fn deserialize_data(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Data::from_bytes(&mut bytes).expect("deserialize data"));
}

#[library_benchmark]
#[bench::size_1k(handle_fixture(1024))]
#[bench::size_64k(handle_fixture(64 * 1024))]
fn serialize_handle(packet: Handle) {
    black_box(ser::to_bytes(&packet).expect("serialize handle"));
}

#[library_benchmark]
#[bench::size_1k(serialized_handle_fixture(1024))]
#[bench::size_64k(serialized_handle_fixture(64 * 1024))]
fn deserialize_handle(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Handle::from_bytes(&mut bytes).expect("deserialize handle"));
}

#[library_benchmark]
#[bench::size_1k(status_fixture(1024))]
#[bench::size_64k(status_fixture(64 * 1024))]
fn serialize_status(packet: Status) {
    black_box(ser::to_bytes(&packet).expect("serialize status"));
}

#[library_benchmark]
#[bench::size_1k(serialized_status_fixture(1024))]
#[bench::size_64k(serialized_status_fixture(64 * 1024))]
fn deserialize_status(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Status::from_bytes(&mut bytes).expect("deserialize status"));
}

#[library_benchmark]
#[bench::entries_1(name_fixture(1))]
#[bench::entries_32(name_fixture(32))]
fn serialize_name(packet: Name) {
    black_box(ser::to_bytes(&packet).expect("serialize name"));
}

#[library_benchmark]
#[bench::entries_1(serialized_name_fixture(1))]
#[bench::entries_32(serialized_name_fixture(32))]
fn deserialize_name(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Name::from_bytes(&mut bytes).expect("deserialize name"));
}

#[library_benchmark]
#[bench::size_1k(open_fixture(1024))]
#[bench::size_64k(open_fixture(64 * 1024))]
fn serialize_open(packet: Open) {
    black_box(ser::to_bytes(&packet).expect("serialize open"));
}

#[library_benchmark]
#[bench::size_1k(serialized_open_fixture(1024))]
#[bench::size_64k(serialized_open_fixture(64 * 1024))]
fn deserialize_open(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Open::from_bytes(&mut bytes).expect("deserialize open"));
}

#[library_benchmark]
#[bench::size_1k(write_packet_fixture(1024))]
#[bench::size_4k(write_packet_fixture(4 * 1024))]
#[bench::size_16k(write_packet_fixture(16 * 1024))]
#[bench::size_64k(write_packet_fixture(64 * 1024))]
fn serialize_write_packet(packet: Packet) {
    black_box(Bytes::try_from(packet).expect("serialize write packet"));
}

#[library_benchmark]
#[bench::size_1k(serialized_write_packet_fixture(1024))]
#[bench::size_4k(serialized_write_packet_fixture(4 * 1024))]
#[bench::size_16k(serialized_write_packet_fixture(16 * 1024))]
#[bench::size_64k(serialized_write_packet_fixture(64 * 1024))]
fn deserialize_write_packet(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Packet::try_from(&mut bytes).expect("deserialize write packet"));
}

#[library_benchmark]
#[bench::size_1k(data_packet_fixture(1024))]
#[bench::size_4k(data_packet_fixture(4 * 1024))]
#[bench::size_16k(data_packet_fixture(16 * 1024))]
#[bench::size_64k(data_packet_fixture(64 * 1024))]
fn serialize_data_packet(packet: Packet) {
    black_box(Bytes::try_from(packet).expect("serialize data packet"));
}

#[library_benchmark]
#[bench::size_1k(serialized_data_packet_fixture(1024))]
#[bench::size_4k(serialized_data_packet_fixture(4 * 1024))]
#[bench::size_16k(serialized_data_packet_fixture(16 * 1024))]
#[bench::size_64k(serialized_data_packet_fixture(64 * 1024))]
fn deserialize_data_packet(serialized: Bytes) {
    let mut bytes = serialized;
    black_box(Packet::try_from(&mut bytes).expect("deserialize data packet"));
}

library_benchmark_group!(
    name = packet_benchmarks;
    benchmarks = serialize_write,
    deserialize_write,
    serialize_data,
    deserialize_data,
    serialize_handle,
    deserialize_handle,
    serialize_status,
    deserialize_status,
    serialize_name,
    deserialize_name,
    serialize_open,
    deserialize_open,
    serialize_write_packet,
    deserialize_write_packet,
    serialize_data_packet,
    deserialize_data_packet
);

main!(library_benchmark_groups = packet_benchmarks);
