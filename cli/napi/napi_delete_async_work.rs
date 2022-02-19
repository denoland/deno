use super::napi_create_async_work::AsyncWork;
use deno_core::napi::*;

/// Frees a previously allocated work object.
#[napi_sym::napi_sym]
fn napi_delete_async_work(env: &mut Env, work: napi_async_work) -> Result {
  let work = Box::from_raw(work);
  drop(work);

  Ok(())
}
