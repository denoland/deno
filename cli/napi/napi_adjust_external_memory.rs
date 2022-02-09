use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_adjust_external_memory(
  env: napi_env,
  _change_in_bytes: i64,
  _adjusted_value: *mut i64,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  // TODO
  Ok(())
}
