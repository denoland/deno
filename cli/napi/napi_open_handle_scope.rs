use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_open_handle_scope(
  env: napi_env,
  result: *mut napi_handle_scope,
) -> Result {
  let env = &mut *(env as *mut Env);
  let isolate = unsafe { &mut **env.isolate_ptr };

  *result = env.scope as *mut _ as napi_handle_scope;
  env.open_handle_scopes += 1;
  Ok(())
}
