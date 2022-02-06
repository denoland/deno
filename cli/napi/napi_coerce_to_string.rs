use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_coerce_to_string(
  env: napi_env,
  value: napi_value,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let coerced = value.to_string(env.scope).unwrap();
  let value: v8::Local<v8::Value> = coerced.into();
  *result = std::mem::transmute(value);
  Ok(())
}
