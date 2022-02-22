use super::napi_create_threadsafe_function::TsFn;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_acquire_threadsafe_function(
  tsfn: napi_threadsafe_function,
  _mode: napi_threadsafe_function_release_mode,
) -> Result {
  let tsfn: &mut TsFn = unsafe { &mut *(tsfn as *mut TsFn) };
  tsfn.acquire()?;

  Ok(())
}
