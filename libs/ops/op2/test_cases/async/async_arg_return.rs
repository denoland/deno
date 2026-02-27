// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(required(1))]
pub async fn op_async(x: i32) -> i32 {
  x
}
