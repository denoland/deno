 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_prototype(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let obj = value.to_object(env.scope).unwrap();
  let proto = obj.get_prototype(env.scope).unwrap();
  *result = std::mem::transmute(proto);
  Ok(())
}
