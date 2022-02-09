use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_threadsafe_function(
  _env: napi_env,
  _func: napi_value,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  _max_queue_size: usize,
  _initial_thread_count: usize,
  _thread_finialize_data: *mut c_void,
  _thread_finalize_cb: napi_finalize,
  _context: *const c_void,
  _call_js_cb: napi_threadsafe_function_call_js,
  _result: *mut napi_threadsafe_function,
) -> Result {
  // TODO
  Ok(())
}
