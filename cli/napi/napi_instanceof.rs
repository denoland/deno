 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_instanceof(
  env: napi_env,
  value: napi_value,
  constructor: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut (env as *mut Env);
  let value: v8::Local<v8::Value> = transmute(value);
  let constructor: v8::Local<v8::Value> = transmute(constructor);
  // TODO: what is the rusty v8 API
  // *result = value.instance_of(constructor);
  Ok(())
}
