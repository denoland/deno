use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_element(
  env: napi_env,
  object: napi_value,
  index: u32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let object: v8::Local<v8::Value> = std::mem::transmute(object);
  let array = v8::Local::<v8::Array>::try_from(object).unwrap();
  let value: v8::Local<v8::Value> = array.get_index(env.scope, index).unwrap();
  *result = std::mem::transmute(value);
  Ok(())
}
