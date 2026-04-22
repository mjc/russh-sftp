# Russh SFTP

SFTP subsystem supported server and client for [Russh](https://github.com/warp-tech/russh) and more!

Crate can provide compatibility with anything that can provide the raw data stream in and out of the subsystem channel.\
Implemented according to [version 3 specifications](https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02) (most popular).

The main idea of the project is to provide an implementation for interacting with the protocol at any level.

## Examples

- [Client example](https://github.com/AspectUnk/russh-sftp/blob/master/examples/client.rs)
- [Simple server](https://github.com/AspectUnk/russh-sftp/blob/master/examples/server.rs)

## Migration note
`server::Handler` remains compatible with string handles and `Vec<u8>` write data when the subsystem is started with `server::run`. To preserve opaque handles and write payloads as `bytes::Bytes`, implement `server::Handler<bytes::Bytes, bytes::Bytes>` and start the subsystem with `server::run_bytes`. Protocol packets store handle and data fields as `bytes::Bytes`; use constructors like `Handle::from_string`, `Write::from_string_vec`, and accessors like `handle_string()` or `data_vec()` when adapting existing string/vector code.

## What's ready?

- [x] Basic packets
- [x] Extended packets
- [x] Simplification for file attributes
- [x] Client side
- [x] Client example
- [x] Server side
- [x] Simple server example
- [ ] Full server example
- [x] Extension support: `limits@openssh.com`, `hardlink@openssh.com`, `fsync@openssh.com`, `statvfs@openssh.com`
- [ ] Unit tests
- [x] Workflow

## Adopters

- [kty](https://github.com/grampelberg/kty) - The terminal for Kubernetes.

## Some words

Thanks to [@Eugeny](https://github.com/Eugeny) (author of the [Russh](https://github.com/warp-tech/russh)) for his prompt help and finalization of Russh API
