use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_open_escapable_handle_scope(
  _env: napi_env,
  _result: *mut napi_escapable_handle_scope,
) -> Result {
  // TODO: do this properly
  Ok(())
}
