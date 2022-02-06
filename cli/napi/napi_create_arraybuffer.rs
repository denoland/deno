use deno_core::napi::*;

use super::util::get_array_buffer_ptr;

#[napi_sym::napi_sym]
fn napi_create_arraybuffer(
  env: napi_env,
  len: usize,
  data: *mut *mut u8,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}
