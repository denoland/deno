use deno_core::napi::*;

// TODO
#[napi_sym::napi_sym]
fn napi_add_finalizer(
  env: napi_env,
  js_object: napi_value,
  native_object: *const c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *const c_void,
  result: *mut napi_ref,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  Ok(())
}
