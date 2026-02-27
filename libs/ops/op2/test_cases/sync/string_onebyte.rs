// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::borrow::Cow;

#[op2(fast)]
fn op_string_onebyte(#[string(onebyte)] s: Cow<[u8]>) -> u32 {
  s.len() as _
}
