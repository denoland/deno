use deno_core::napi::*;

#[repr(C)]
pub struct AsyncWork {
  pub data: *mut c_void,
  pub execute: napi_async_execute_callback,
  pub complete: napi_async_complete_callback,
}

unsafe impl Send for AsyncWork {}
unsafe impl Sync for AsyncWork {}

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
