 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_undefined(env: napi_env, result: *mut napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::undefined(env.scope).into();
  *result = std::mem::transmute(value);
  Ok(())
}
