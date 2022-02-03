 
use deno_core::napi::*;

use super::util::get_array_buffer_ptr;

#[napi_sym::napi_sym]
fn napi_create_dataview(
  env: napi_env,
  len: usize,
  data: *mut *mut u8,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value = v8::ArrayBuffer::new(env.scope, len);
  if !data.is_null() {
    *data = get_array_buffer_ptr(value);
  }
  let context = env.scope.get_current_context();
  let global = context.global(env.scope);
  let data_view_name = v8::String::new(env.scope, "DataView").unwrap();
  let data_view = global.get(env.scope, data_view_name.into()).unwrap();
  let data_view = v8::Local::<v8::Function>::try_from(data_view).unwrap();
  let byte_offset = v8::Number::new(env.scope, byte_offset as f64);
  let byte_length = v8::Number::new(env.scope, len as f64);
  let value = data_view
    .new_instance(
      env.scope,
      &[value.into(), byte_offset.into(), byte_length.into()],
    )
    .unwrap();
  let value: v8::Local<v8::Value> = value.into();
  *result = std::mem::transmute(value);
  Ok(())
}
