# deno_kv

This crate provides a key/value store for Deno. For an overview of Deno KV,
please read the [manual](https://docs.deno.com/deploy/kv/manual).

## Storage Backends

Deno KV has a pluggable storage interface that supports multiple backends:

- SQLite - backed by a local SQLite database. This backend is suitable for
  development and is the default when running locally. It is implemented in the
  [denokv_sqlite crate](https://github.com/denoland/denokv/blob/main/sqlite).
- Remote - backed by a remote service that implements the
  [KV Connect](#kv-connect) protocol, for example
  [Deno Deploy](https://deno.com/deploy).

Additional backends can be added by implementing the `Database` trait.

## KV Connect

The KV Connect protocol allows the Deno CLI to communicate with a remote KV
database. The
[specification for the protocol](https://github.com/denoland/denokv/blob/main/proto/kv-connect.md),
and the
[protobuf definitions](https://github.com/denoland/denokv/blob/main/proto/schema/datapath.proto)
can be found in the `denokv` repository, under the `proto` directory.
