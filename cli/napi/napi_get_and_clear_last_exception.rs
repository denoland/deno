use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_and_clear_last_exception(
  env: napi_env,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  // TODO: just return undefined for now we don't cache
  // exceptions in env.
  let value: v8::Local<v8::Value> = v8::undefined(env.scope).into();
  *result = std::mem::transmute(value);
  Ok(())
}
