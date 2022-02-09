use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_is_dataview(
  _env: napi_env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let value: v8::Local<v8::Value> = transmute(value);
  *result = value.is_data_view();
  Ok(())
}
