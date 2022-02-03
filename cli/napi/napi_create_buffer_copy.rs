 
use deno_core::napi::*;

use super::util::get_array_buffer_ptr;

#[napi_sym::napi_sym]
fn napi_create_buffer_copy(
  env: napi_env,
  len: usize,
  data: *mut u8,
  result_data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  let ptr = get_array_buffer_ptr(value);
  std::ptr::copy(data, ptr, len);
  if !result_data.is_null() {
    *result_data = ptr;
  }
  let value = v8::Uint8Array::new(env.scope, value, 0, len).unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}
