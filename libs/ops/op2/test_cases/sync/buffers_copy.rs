// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(fast)]
fn op_buffers(
  #[buffer(copy)] _a: Vec<u8>,
  #[buffer(copy)] _b: Box<[u8]>,
  #[buffer(copy)] _c: bytes::Bytes,
) {
}

#[op2(fast)]
fn op_buffers_32(#[buffer(copy)] _a: Vec<u32>, #[buffer(copy)] _b: Box<[u32]>) {
}
