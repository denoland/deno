// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_error::JsErrorBox;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

// Collect a few examples that we'll smoke test when not running on the CI.

#[op2]
pub async fn op_async1() {}

#[op2]
pub async fn op_async2(x: i32) -> i32 {
  x
}

#[op2]
pub async fn op_async3(x: i32) -> std::io::Result<i32> {
  Ok(x)
}

#[op2]
pub fn op_async4(x: i32) -> Result<impl Future<Output = i32>, JsErrorBox> {
  Ok(async move { x })
}

#[op2]
pub fn op_async5(
  x: i32,
) -> Result<impl Future<Output = std::io::Result<i32>>, JsErrorBox> {
  Ok(async move { Ok(x) })
}

#[op2]
pub async fn op_async6(x: f32) -> f32 {
  x
}

#[op2]
pub async fn op_async_opstate(
  state: Rc<RefCell<OpState>>,
) -> std::io::Result<i32> {
  Ok(*state.borrow().borrow::<i32>())
}

#[op2]
#[buffer]
pub async fn op_async_buffer(#[buffer] buf: JsBuffer) -> JsBuffer {
  buf
}

#[op2]
#[string]
pub async fn op_async_string(#[string] s: String) -> String {
  s
}
