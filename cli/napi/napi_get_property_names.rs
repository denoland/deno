 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_property_names(
  env: napi_env,
  object: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object: v8::Local<v8::Value> = std::mem::transmute(object);
  let array: v8::Local<v8::Array> = object
    .to_object(env.scope)
    .unwrap()
    .get_property_names(env.scope)
    .unwrap();
  let value: v8::Local<v8::Value> = array.into();
  *result = std::mem::transmute(value);
  Ok(())
}
