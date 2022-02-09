use super::util::get_array_buffer_ptr;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_arraybuffer_info(
  _env: napi_env,
  value: napi_value,
  data: *mut *mut u8,
  length: *mut usize,
) -> Result {
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let buf = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
  if !data.is_null() {
    *data = get_array_buffer_ptr(buf);
  }
  *length = buf.byte_length();
  Ok(())
}
