use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_is_exception_pending(env: napi_env, result: *mut bool) -> Result {
  let mut env = &mut *(env as *mut Env);
  // TODO
  *result = false;
  Ok(())
}
