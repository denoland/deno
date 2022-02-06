use super::util::get_array_buffer_ptr;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_dataview_info(
  env: napi_env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let buf = v8::Local::<v8::DataView>::try_from(value).unwrap();
  let buffer_name = v8::String::new(env.scope, "buffer").unwrap();
  let abuf = v8::Local::<v8::ArrayBuffer>::try_from(
    buf.get(env.scope, buffer_name.into()).unwrap(),
  )
  .unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(abuf);
  }
  *length = abuf.byte_length();
  Ok(())
}
