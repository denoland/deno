 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_threadsafe_function(
  env: napi_env,
  func: napi_value,
  async_resource: napi_value,
  async_resource_name: napi_value,
  max_queue_size: usize,
  initial_thread_count: usize,
  thread_finialize_data: *mut c_void,
  thread_finalize_cb: napi_finalize,
  context: *const c_void,
  call_js_cb: napi_threadsafe_function_call_js,
  result: *mut napi_threadsafe_function,
) -> Result {
  let env = &mut *(env as *mut Env);
  // TODO
  Ok(())
}
