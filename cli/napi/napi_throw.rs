 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_throw(env: napi_env, error: napi_value) -> Result {
  let mut env = &mut *(env as *mut Env);
  let error = transmute(error);
  env.scope.throw_exception(error);
  Ok(())
}
