use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_delete_async_work(_env: napi_env, _work: napi_async_work) -> Result {
  // TODO
  Ok(())
}
