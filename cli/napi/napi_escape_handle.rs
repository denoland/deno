use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_escape_handle(
  _env: napi_env,
  _handle_scope: napi_escapable_handle_scope,
  _escapee: napi_value,
  _result: *mut napi_value,
) -> Result {
  // TODO
  Ok(())
}
