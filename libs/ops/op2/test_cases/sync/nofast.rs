// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(nofast)]
fn op_nofast(a: u32, b: u32) -> u32 {
  a + b
}
