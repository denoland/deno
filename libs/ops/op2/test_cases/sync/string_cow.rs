// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::borrow::Cow;

#[op2(fast)]
fn op_string_cow(#[string] s: Cow<str>) -> u32 {
  s.len() as _
}
