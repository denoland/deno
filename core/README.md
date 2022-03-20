# Deno Core Crate

[![crates](https://img.shields.io/crates/v/deno_core.svg)](https://crates.io/crates/deno_core)
[![docs](https://docs.rs/deno_core/badge.svg)](https://docs.rs/deno_core)

The main dependency of this crate is
[rusty_v8](https://github.com/denoland/rusty_v8), which provides the V8-Rust
bindings.

This Rust crate contains the essential V8 bindings for Deno's command-line
interface (Deno CLI). The main abstraction here is the JsRuntime which provides
a way to execute JavaScript.

The JsRuntime implements an event loop abstraction for the executed code that
keeps track of all pending tasks (async ops, dynamic module loads). It is user's
responsibility to drive that loop by using `JsRuntime::run_event_loop` method -
it must be executed in the context of Rust's future executor (eg. tokio, smol).

Rust functions can be registered in JavaScript using `deno_core::Extension`. Use
the `Deno.core.opSync()` and `Deno.core.opAsync()` functions to trigger the op
function callback. A conventional way to write ops is using the
[`deno_ops`](https://github.com/denoland/deno/blob/main/ops) crate.

Documentation for this crate is thin at the moment. Please see
[hello_world.rs](https://github.com/denoland/deno/blob/main/core/examples/hello_world.rs)
and
[http_bench_json_ops.rs](https://github.com/denoland/deno/blob/main/core/examples/http_bench_json_ops.rs)
as examples of usage.

TypeScript support and lots of other functionality are not available at this
layer. See the [CLI](https://github.com/denoland/deno/tree/main/cli) for that.
