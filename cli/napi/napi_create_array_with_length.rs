 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_array_with_length(
  env: napi_env,
  len: i32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Array::new(env.scope, len).into();
  *result = std::mem::transmute(value);
  Ok(())
}
