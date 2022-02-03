 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_is_error(
  env: napi_env,
  value: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut (env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  // TODO
  *result = value.is_object();
  Ok(())
}
