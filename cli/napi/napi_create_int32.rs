use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_int32(
  env: napi_env,
  value: i32,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> =
    v8::Number::new(env.scope, value as f64).into();
  *result = std::mem::transmute(value);
  Ok(())
}
