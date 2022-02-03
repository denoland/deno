 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_has_property(
  env: napi_env,
  value: napi_value,
  key: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = transmute(value);
  let obj = value.to_object(env.scope).unwrap();
  *result = obj
    .has(env.scope, std::mem::transmute(key))
    .unwrap_or(false);
  Ok(())
}
