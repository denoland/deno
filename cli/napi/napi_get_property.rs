 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_property(
  env: napi_env,
  object: napi_value,
  key: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object: v8::Local<v8::Value> = std::mem::transmute(object);
  let object = object.to_object(env.scope).unwrap();
  let key: v8::Local<v8::Value> = std::mem::transmute(key);
  let value: v8::Local<v8::Value> = object.get(env.scope, key).unwrap();
  *result = std::mem::transmute(value);
  Ok(())
}
