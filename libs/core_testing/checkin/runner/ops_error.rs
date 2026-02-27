// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_error::JsErrorBox;

#[op2]
pub async fn op_async_throw_error_eager() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::type_error("Error"))
}

#[op2(async(deferred), fast)]
pub async fn op_async_throw_error_deferred() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::type_error("Error"))
}

#[op2(async(lazy), fast)]
pub async fn op_async_throw_error_lazy() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::type_error("Error"))
}

#[op2(fast)]
pub fn op_error_custom_sync(
  #[string] message: String,
) -> Result<(), JsErrorBox> {
  Err(JsErrorBox::new("BadResource", message))
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[error("{message}")]
struct MyError {
  message: String,
  #[property]
  code: u32,
}

#[op2(fast)]
pub fn op_error_custom_with_code_sync(
  #[string] message: String,
  code: u32,
) -> Result<(), MyError> {
  Err(MyError { message, code })
}
