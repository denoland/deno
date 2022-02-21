use super::napi_create_threadsafe_function::TsFn;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_release_threadsafe_function(
  tsfn: napi_threadsafe_function,
  mode: napi_threadsafe_function_release_mode,
) -> Result {
  let tsfn: Box<TsFn> = unsafe { Box::from_raw(tsfn as *mut TsFn) };
  tsfn.release()?;

  Ok(())
}
