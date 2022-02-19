use super::napi_create_async_work::AsyncWork;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_queue_async_work(env: napi_env, work: napi_async_work) -> Result {
  let work = transmute::<napi_async_work, Box<AsyncWork>>(work);

  let env_ptr = &mut *(env as *mut Env);
  let sender = env_ptr.async_work_sender.clone();
  let isolate_ptr = env_ptr.isolate_ptr;
  let fut = Box::new(move |scope: &mut v8::HandleScope| {
    let mut env = Env::new(isolate_ptr, scope, sender);
    let env_ptr = &mut env as *mut _ as napi_env;
    (work.execute)(env_ptr, work.data);

    // Note: Must be called from the loop thread.
    (work.complete)(env_ptr, napi_ok, work.data);
  });
  env_ptr.add_async_work(fut);

  Ok(())
}
