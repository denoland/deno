use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_cancel_async_work(
  _env: napi_env,
  _async_work: napi_async_work,
) -> Result {
  // TODO
  Ok(())
}
