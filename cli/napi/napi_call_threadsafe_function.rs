use super::napi_create_threadsafe_function::TsFn;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_call_threadsafe_function(
  func: napi_threadsafe_function,
  data: *mut c_void,
  is_blocking: napi_threadsafe_function_call_mode,
) -> Result {
  let tsfn: &TsFn = unsafe { &*(func as *const TsFn) };
  let _func = tsfn.call(data, is_blocking != 0);

  Ok(())
}
