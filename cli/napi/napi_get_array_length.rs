use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_array_length(
  _env: napi_env,
  value: napi_value,
  result: *mut u32,
) -> Result {
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  *result = v8::Local::<v8::Array>::try_from(value).unwrap().length();
  Ok(())
}
