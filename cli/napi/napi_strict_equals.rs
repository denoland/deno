 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_strict_equals(
  env: napi_env,
  lhs: napi_value,
  rhs: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut (env as *mut Env);
  let lhs: v8::Local<v8::Value> = std::mem::transmute(lhs);
  let rhs: v8::Local<v8::Value> = std::mem::transmute(rhs);
  *result = lhs.strict_equals(rhs);
  Ok(())
}
