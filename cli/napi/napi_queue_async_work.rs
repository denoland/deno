use super::napi_create_async_work::AsyncWork;
use deno_core::napi::*;
use std::sync::Arc;
use std::sync::Mutex;

#[napi_sym::napi_sym]
fn napi_queue_async_work(env: napi_env, work: napi_async_work) -> Result {
  let env_ptr = &mut *(env as *mut Env);
  let work = transmute::<napi_async_work, Box<AsyncWork>>(work);

  let handle = tokio::task::spawn_local(async move {
    let env = transmute(env_ptr);
    (work.execute)(env, work.data);

    // Note: Must be called from the loop thread.
    (work.complete)(env, napi_ok, work.data);
  });

  Ok(())
}
