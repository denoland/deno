 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_is_typedarray(
  env: napi_env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = transmute(value);
  *result = value.is_typed_array();
  Ok(())
}
