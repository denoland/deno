# `deno_runtime` crate

[![crates](https://img.shields.io/crates/v/deno_runtime.svg)](https://crates.io/crates/deno_runtime)
[![docs](https://docs.rs/deno_runtime/badge.svg)](https://docs.rs/deno_runtime)

This crate provides an implementation of Deno runtime including:

- APIs available on `Deno` namespace along with accompying ops
- permission checks
- inspector/debugger support

The main API of this crate is `MainWorker` - a top level runtime, which contains
`window` and `Deno` globals.

`MainWorker` is highly configurable and allows to customize many of the
runtime's properties:

- module loading implementation
- error formatting
- support for source maps
- support for V8 inspector and Chrome Devtools debugger
- HTTP client user agent, CA certificate
- random number generator seed

`MainWorker` comes with a built-in support for `Worker` Web API. The `Worker`
API is implemented using `WebWorker` structure, that creates a dedicated OS
thread.

When creating a new instance of `MainWorker` implementors must provide a
callback that is called when creating a new instance of `WebWorker`.

## Note

This crate is agnostic of TypeScript.

It is possible to provide TS support using `module_loader` property on
`WorkerOptions`. (for more details see the
[CLI](https://github.com/denoland/deno/tree/master/cli))
