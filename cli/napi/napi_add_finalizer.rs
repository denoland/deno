use deno_core::napi::*;

// TODO
#[napi_sym::napi_sym]
fn napi_add_finalizer(
  _env: napi_env,
  _js_object: napi_value,
  _native_object: *const c_void,
  _finalize_cb: napi_finalize,
  _finalize_hint: *const c_void,
  _result: *mut napi_ref,
) -> Result {
  Ok(())
}
