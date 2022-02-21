use super::napi_create_threadsafe_function::TsFn;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_unref_threadsafe_function(
  env: &mut Env,
  tsfn: napi_threadsafe_function,
) -> Result {
  let tsfn: &TsFn = unsafe { &*(tsfn as *const TsFn) };

  Ok(())
}
