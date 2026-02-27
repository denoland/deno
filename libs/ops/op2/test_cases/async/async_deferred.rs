// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(async(deferred), fast)]
pub async fn op_async_deferred() -> std::io::Result<i32> {
  Ok(0)
}
