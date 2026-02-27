// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(fast)]
fn op_string_owned(#[string] s: &str) -> u32 {
  s.len() as _
}
