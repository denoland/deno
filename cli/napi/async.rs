use deno_core::napi::*;

#[repr(C)]
pub struct AsyncWork {
  pub data: *mut c_void,
  pub execute: napi_async_execute_callback,
  pub complete: napi_async_complete_callback,
}

#[napi_sym::napi_sym]
fn napi_create_async_work(
  _env: napi_env,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  execute: napi_async_execute_callback,
  complete: napi_async_complete_callback,
  data: *mut c_void,
  result: *mut napi_async_work,
) -> Result {
  let mut work = AsyncWork {
    data,
    execute,
    complete,
  };
  *result = transmute::<Box<AsyncWork>, _>(Box::new(work));
  Ok(())
}

#[napi_sym::napi_sym]
fn napi_cancel_async_work(
  _env: &mut Env,
  _async_work: napi_async_work,
) -> Result {
  Ok(())
}

/// Frees a previously allocated work object.
#[napi_sym::napi_sym]
fn napi_delete_async_work(_env: &mut Env, work: napi_async_work) -> Result {
  let work = Box::from_raw(work);
  drop(work);

  Ok(())
}

#[napi_sym::napi_sym]
fn napi_queue_async_work(env: napi_env, work: napi_async_work) -> Result {
  let work: &AsyncWork = unsafe { &*(work as *const AsyncWork) };

  let env_ptr = &mut *(env as *mut Env);
  let sender = env_ptr.async_work_sender.clone();
  let tsfn_sender = env_ptr.threadsafe_function_sender.clone();
  let isolate_ptr = env_ptr.isolate_ptr;
  let fut = Box::new(move |scope: &mut v8::HandleScope| {
    let ctx = scope.get_current_context();
    let ctx = v8::Global::new(scope, ctx);
    let mut env = Env::new(
      isolate_ptr,
      ctx.clone(),
      sender.clone(),
      tsfn_sender.clone(),
    );
    let env_ptr = &mut env as *mut _ as napi_env;
    (work.execute)(env_ptr, work.data);
    // TODO: Clean this up...
    env.add_async_work(Box::new(move |scope: &mut v8::HandleScope| {
      let mut env = Env::new(isolate_ptr, ctx, sender, tsfn_sender);
      let env_ptr = &mut env as *mut _ as napi_env;
      // Note: Must be called from the loop thread.
      (work.complete)(env_ptr, napi_ok, work.data);
    }));
    std::mem::forget(env);
  });
  env_ptr.add_async_work(fut);

  Ok(())
}
