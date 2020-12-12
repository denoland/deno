# `deno_runtime` crate

[![crates](https://img.shields.io/crates/v/deno_runtime.svg)](https://crates.io/crates/deno_runtime)
[![docs](https://docs.rs/deno_runtime/badge.svg)](https://docs.rs/deno_runtime)

This crate provides implementation of `Deno` runtime.

It's comprised of few major parts:

- runtime implementation consisting of:
  - JavaScript code, in `rt/` directory, that implements `Deno` APIs (eg.
    `Deno.listen()`)
  - Rust code, in `ops/` module, that implements ops for `Deno` APIs (eg.
    `op_listen`)
  - cargo build script (`build.rs`), that creates a V8 snapshot of the runtime
    code for fast startup
- permission checker implementation
- V8 inspector and debugger support

This crate doesn't support TypeScript out-of-the-box.

It is possible to provide TS support using `module_loader` property on
`WorkerOptions`, for more details see the
[CLI](https://github.com/denoland/deno/tree/master/cli) crate.

As a consequence following `Deno` JavaScript APIs have no corresponding op
implementation in Rust:

- `Deno.applySourceMaps`
- `Deno.bundle`
- `Deno.compile`
- `Deno.formatDiagnostics`

Op implementation for these APIs can be found in
[CLI](https://github.com/denoland/deno/tree/master/cli).

## Stability

This crate is built using battle-tested modules that were originally in `deno`
crate, however the API of this crate is subject to rapid and breaking changes.

## `MainWorker`

The main API of this crate is `MainWorker`.

`MainWorker` is a structure encapsulating `deno_core::JsRuntime` with a set of
ops used to implement `Deno` namespace.

When creating a `MainWorker` implementors must call `MainWorker::bootstrap` to
prepare JS runtime for use.

`MainWorker` is highly configurable and allows to customize many of the
runtime's properties:

- module loading implementation
- error formatting
- support for source maps
- support for V8 inspector and Chrome Devtools debugger
- HTTP client user agent, CA certificate
- random number generator seed

## `Worker` Web API

`deno_runtime` comes with a built-in support for `Worker` Web API.

The `Worker` API is implemented using `WebWorker` structure.

When creating a new instance of `MainWorker` implementors must provide a
callback function that is used when creating a new instance of `Worker`.

All `WebWorker` instances are decendents of `MainWorker` which is responsible
for setting up communication with child worker. Each `WebWorker` spawns a new OS
thread that is dedicated solely to that worker.
