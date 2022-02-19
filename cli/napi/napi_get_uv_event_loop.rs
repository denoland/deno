use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_uv_event_loop(env: &mut Env, uv_loop: *mut *mut ()) -> Result {
  // Don't error out because addons maybe pass this to
  // our libuv _polyfills_.
  *uv_loop = std::ptr::null_mut();
  Ok(())
}
