use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_value_int32(
  env: napi_env,
  value: napi_value,
  result: *mut i32,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  *result = value.int32_value(env.scope).unwrap();
  Ok(())
}
