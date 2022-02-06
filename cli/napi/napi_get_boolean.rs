use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_boolean(
  env: napi_env,
  value: bool,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::Boolean::new(env.scope, value).into();
  *result = std::mem::transmute(value);
  Ok(())
}
