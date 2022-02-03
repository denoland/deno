 
use deno_core::napi::*;
use super::napi_create_async_work::AsyncWork;
use std::sync::Arc;
use std::sync::Mutex;

#[napi_sym::napi_sym]
fn napi_queue_async_work(env: napi_env, work: napi_async_work) -> Result {
  let env_ptr = &mut *(env as *mut Env);
  let work = transmute::<napi_async_work, Box<AsyncWork>>(work);

  let data = Arc::new(Mutex::new(work));
  let (tx, rx) = std::sync::mpsc::channel::<()>();

  let shared = Arc::clone(&data);
  tokio::task::spawn_blocking(move || {
    let env = transmute(env_ptr);
    let work = shared.lock().unwrap();
    (work.execute)(env, work.data);
    tx.send(()).unwrap();
  });

  // Note: Must be called from the loop thread.
  // TODO: Don't block the loop thread.
  rx.recv().unwrap();
  let work = data.lock().unwrap();
  (work.complete)(env, napi_ok, work.data);
  Ok(())
}
