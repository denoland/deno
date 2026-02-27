// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_error::JsErrorBox;

#[op2(fast)]
pub fn op_u32_with_result() -> Result<u32, JsErrorBox> {
  Ok(0)
}
