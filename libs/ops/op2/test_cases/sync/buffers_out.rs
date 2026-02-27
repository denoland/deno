// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::JsBuffer;

#[op2]
#[buffer]
fn op_buffers(#[buffer] buffer: JsBuffer) -> JsBuffer {
  buffer
}

#[op2]
#[buffer]
fn op_buffers_option(#[buffer] buffer: Option<JsBuffer>) -> Option<JsBuffer> {
  buffer
}

#[op2]
#[arraybuffer]
fn op_arraybuffers(#[arraybuffer] buffer: JsBuffer) -> JsBuffer {
  buffer
}

// TODO(mmastrac): Option + Marker doesn't work yet

// #[op2]
// #[arraybuffer]
// fn op_arraybuffers_option(#[arraybuffer] buffer: Option<JsBuffer>) -> Option<JsBuffer> {
//   buffer
// }
