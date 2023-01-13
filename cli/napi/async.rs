// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi::*;

#[repr(C)]
pub struct AsyncWork {
  pub data: *mut c_void,
  pub execute: napi_async_execute_callback,
  pub complete: napi_async_complete_callback,
}

#[napi_sym::napi_sym]
fn napi_create_async_work(
  _env: *mut Env,
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
fn napi_queue_async_work(env_ptr: *mut Env, work: napi_async_work) -> Result {
  let work: &AsyncWork = &*(work as *const AsyncWork);
  let env: &mut Env = env_ptr.as_mut().ok_or(Error::InvalidArg)?;

  let fut = Box::new(move || {
    (work.execute)(env_ptr as napi_env, work.data);
    // Note: Must be called from the loop thread.
    (work.complete)(env_ptr as napi_env, napi_ok, work.data);
  });
  env.add_async_work(fut);

  Ok(())
}

// TODO: Custom async operations.

#[napi_sym::napi_sym]
fn napi_async_init(
  _env: *mut Env,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  _result: *mut *mut (),
) -> Result {
  todo!()
}

#[napi_sym::napi_sym]
fn napi_async_destroy(_env: *mut Env, _async_context: *mut ()) -> Result {
  todo!()
}
