use super::napi_create_threadsafe_function::TsFn;
use deno_core::napi::*;

/// Maybe called from any thread.
#[napi_sym::napi_sym]
pub fn napi_get_threadsafe_function_context(
  func: napi_threadsafe_function,
  result: *mut *const c_void,
) -> Result {
  let tsfn: &TsFn = unsafe { &*(func as *const TsFn) };
  *result = tsfn.context;
  Ok(())
}
