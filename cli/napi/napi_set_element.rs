 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_set_element(
  env: napi_env,
  object: napi_value,
  index: u32,
  value: napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object: v8::Local<v8::Value> = std::mem::transmute(object);
  let array = v8::Local::<v8::Array>::try_from(object).unwrap();
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  array.set_index(env.scope, index, value).unwrap();
  Ok(())
}
