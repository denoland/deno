 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_has_element(
  env: napi_env,
  value: napi_value,
  index: u32,
  result: *mut bool,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let obj = value.to_object(env.scope).unwrap();
  *result = obj.has_index(env.scope, index).unwrap_or(false);
  Ok(())
}
