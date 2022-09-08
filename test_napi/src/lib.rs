// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi_derive::napi;

#[napi]
pub fn test_undefined() -> Undefined {}

#[napi]
pub fn test_null() -> Null {
  Null
}

#[napi]
pub fn test_int32() -> i32 {
  69
}

#[napi]
pub fn test_int64() -> i64 {
  i64::MAX
}

#[napi]
pub fn test_string(name: String) -> String {
  format!("Hello, {}!", name)
}

#[napi]
pub fn test_bool() -> bool {
  true
}

#[napi]
pub fn test_typedarray(input: Uint8Array) -> Uint8Array {
  let mut input: Vec<u8> = input.to_vec();
  input.reverse();
  Uint8Array::new(input)
}

#[napi]
pub fn test_create_obj(env: Env) -> Object {
  let mut obj = env.create_object().unwrap();
  obj.set("test", 1).unwrap();
  obj
}

#[napi]
pub fn test_get_field(obj: Object, field: String) -> String {
  obj.get::<&str, String>(&field).unwrap().unwrap()
}

#[napi]
pub fn test_arr_len(arr: Array) -> u32 {
  arr.len()
}

#[napi(js_name = "ObjectWrap")]
pub struct ObjectWrap {
  value: i32,
}

#[napi]
impl ObjectWrap {
  #[napi(constructor)]
  pub fn new(count: i32) -> Self {
    Self { value: count }
  }

  #[napi(getter)]
  pub fn get_value(&self) -> i32 {
    self.value
  }

  #[napi]
  pub fn set_value(&mut self, value: i32) {
    self.value = value;
  }
}

#[napi]
pub fn call_threadsafe_function(callback: JsFunction) -> Result<()> {
  let tsfn: ThreadsafeFunction<u32, ErrorStrategy::CalleeHandled> = callback
    .create_threadsafe_function(0, |ctx| {
      ctx.env.create_uint32(ctx.value + 1).map(|v| vec![v])
    })?;
  for n in 0..100 {
    let tsfn = tsfn.clone();
    std::thread::spawn(move || {
      tsfn.call(Ok(n), ThreadsafeFunctionCallMode::NonBlocking);
    });
  }
  Ok(())
}

#[napi]
pub async fn read_file_async(path: String) -> Result<Buffer> {
  let r = tokio::fs::read(path).await;

  match r {
    Ok(content) => Ok(content.into()),
    Err(e) => Err(Error::new(
      Status::GenericFailure,
      format!("failed to read file, {}", e),
    )),
  }
}
