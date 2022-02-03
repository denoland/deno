 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_is_buffer(
  env: napi_env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut (env as *mut Env);
  let value: v8::Local<v8::Value> = transmute(value);
  // TODO: should we assume Buffer as Uint8Array in Deno?
  // or use std/node polyfill?
  *result = value.is_typed_array();
  Ok(())
}
