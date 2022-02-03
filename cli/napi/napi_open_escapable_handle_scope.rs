use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_open_escapable_handle_scope(
  env: napi_env,
  result: *mut napi_escapable_handle_scope,
) -> Result {
  // TODO: do this properly
  Ok(())
}
