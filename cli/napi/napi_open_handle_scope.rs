use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_open_handle_scope(
  env: napi_env,
  result: *mut napi_handle_scope,
) -> Result {
  let env = &mut *(env as *mut Env);
  let scope = &mut v8::HandleScope::new(env.scope);
  *result = transmute(scope);
  env.open_handle_scopes += 1;
  Ok(())
}
