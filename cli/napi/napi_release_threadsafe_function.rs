use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_release_threadsafe_function() -> Result {
  Ok(())
}
