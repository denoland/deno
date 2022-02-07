use super::napi_create_async_work::AsyncWork;
use deno_core::futures::FutureExt;
use deno_core::napi::*;
use std::sync::Arc;
use std::sync::Mutex;

#[napi_sym::napi_sym]
fn napi_queue_async_work(env: napi_env, work: napi_async_work) -> Result {
  let work = transmute::<napi_async_work, Box<AsyncWork>>(work);
  
  let fut = async move {
    (work.execute)(env, work.data);

    // Note: Must be called from the loop thread.
    (work.complete)(env, napi_ok, work.data);
  }
  .boxed_local();
  let env_ptr = &mut *(env as *mut Env);
  env_ptr.add_async_work(fut);

  Ok(())
}
