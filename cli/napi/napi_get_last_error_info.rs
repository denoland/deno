use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_last_error_info(
  _env: napi_env,
  error_code: *mut *const napi_extended_error_info,
) -> Result {
  // let err_info = napi_extended_error_info {
  //   error_message: std::ptr::null(),
  //   engine_reserved: std::ptr::null_mut(),
  //   engine_error_code: 0,
  //   status_code: napi_ok,
  // };

  // *error_code = &err_info as *const napi_extended_error_info;
  *error_code = std::ptr::null();
  Ok(())
}
