use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_close_escapable_handle_scope(
  env: napi_env,
  scope: napi_escapable_handle_scope,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  // TODO: do this properly
  Ok(())
}
