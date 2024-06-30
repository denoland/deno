// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;

#[op2(fast)]
pub fn op_is_ascii(#[buffer] buf: &[u8]) -> bool {
  buf.is_ascii()
}

#[op2(fast)]
pub fn op_is_utf8(#[buffer] buf: &[u8]) -> bool {
  std::str::from_utf8(buf).is_ok()
}
