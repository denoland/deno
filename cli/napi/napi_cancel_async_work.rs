use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_cancel_async_work(
  env: &mut Env,
  async_work: napi_async_work,
) -> Result {
  Ok(())
}
