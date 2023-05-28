// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi::*;

use crate::check_env;

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
) -> napi_status {
  let mut work = AsyncWork {
    data,
    execute,
    complete,
  };
  let work_box = Box::new(work);
  *result = transmute::<*mut AsyncWork, _>(Box::into_raw(work_box));
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_cancel_async_work(
  _env: &mut Env,
  _async_work: napi_async_work,
) -> napi_status {
  napi_ok
}

/// Frees a previously allocated work object.
#[napi_sym::napi_sym]
fn napi_delete_async_work(
  _env: &mut Env,
  work: napi_async_work,
) -> napi_status {
  let work = Box::from_raw(work as *mut AsyncWork);
  drop(work);

  napi_ok
}

#[napi_sym::napi_sym]
fn napi_queue_async_work(
  env_ptr: *mut Env,
  work: napi_async_work,
) -> napi_status {
  let work: &AsyncWork = &*(work as *const AsyncWork);
  let Some(env) = env_ptr.as_mut() else {
    return napi_invalid_arg;
  };

  let fut = Box::new(move || {
    (work.execute)(env_ptr as napi_env, work.data);
    // Note: Must be called from the loop thread.
    (work.complete)(env_ptr as napi_env, napi_ok, work.data);
  });
  env.add_async_work(fut);

  napi_ok
}

// NOTE: we don't support "async_hooks::AsyncContext" so these APIs are noops.
#[napi_sym::napi_sym]
fn napi_async_init(
  env: *mut Env,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  result: *mut *mut (),
) -> napi_status {
  check_env!(env);
  *result = ptr::null_mut();
  napi_ok
}

#[napi_sym::napi_sym]
fn napi_async_destroy(env: *mut Env, async_context: *mut ()) -> napi_status {
  check_env!(env);
  assert!(async_context.is_null());
  napi_ok
}
