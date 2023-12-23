// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::op2;
use deno_core::v8;

deno_core::extension!(
  deno_webidl,
  ops = [
    op_is_array_buffer,
    op_is_shared_array_buffer,
  ],
  esm = ["00_webidl.js"],
);

#[op2(fast)]
pub fn op_is_array_buffer(value: &v8::Value) -> bool {
  value.is_array_buffer()
}

#[op2(fast)]
pub fn op_is_shared_array_buffer(value: &v8::Value) -> bool {
  value.is_shared_array_buffer()
}
