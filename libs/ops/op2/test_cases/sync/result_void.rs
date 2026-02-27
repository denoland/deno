// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(fast)]
pub fn op_void_with_result() -> std::io::Result<()> {
  Ok(())
}
