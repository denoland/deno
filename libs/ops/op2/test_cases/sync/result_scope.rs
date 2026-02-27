// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;
use deno_error::JsErrorBox;

#[op2(fast)]
pub fn op_void_with_result(
  _scope: &mut v8::PinScope,
) -> Result<(), JsErrorBox> {
  Ok(())
}
