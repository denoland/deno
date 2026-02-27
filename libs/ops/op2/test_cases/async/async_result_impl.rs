// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_error::JsErrorBox;
use std::future::Future;

#[op2]
pub fn op_async_result_impl(
  x: i32,
) -> Result<impl Future<Output = std::io::Result<i32>>, JsErrorBox> {
  Ok(async move { Ok(x) })
}
