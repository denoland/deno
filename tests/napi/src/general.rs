// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::CStr;
use std::ptr;

use napi_sys::Status::napi_ok;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

/// Test napi_instanceof.
extern "C" fn test_instanceof(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut result = false;
  assert_napi_ok!(napi_instanceof(env, args[0], args[1], &mut result));

  let mut val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, result, &mut val));
  val
}

/// Test napi_get_version.
extern "C" fn test_get_version(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut version: u32 = 0;
  assert_napi_ok!(napi_get_version(env, &mut version));

  // NAPI version should be at least 1
  assert!(version >= 1);

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_uint32(env, version, &mut result));
  result
}

/// Test napi_run_script: evaluates a string as JS and returns the result.
extern "C" fn test_run_script(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_run_script(env, args[0], &mut result));
  result
}

/// Test napi_get_node_version.
extern "C" fn test_get_node_version(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut node_version: *const napi_node_version = ptr::null();
  assert_napi_ok!(napi_get_node_version(env, &mut node_version));

  assert!(!node_version.is_null());
  let ver = unsafe { &*node_version };
  // Major version should be reasonable (>= 1)
  assert!(ver.major >= 1);

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_uint32(env, ver.major, &mut result));
  result
}

/// Test napi_get_last_error_info.
extern "C" fn test_get_last_error_info(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // First, call napi_get_last_error_info after a successful operation.
  // The error_code should be napi_ok.
  let mut version: u32 = 0;
  assert_napi_ok!(napi_get_version(env, &mut version));

  let mut error_info: *const napi_extended_error_info = ptr::null();
  assert_napi_ok!(napi_get_last_error_info(env, &mut error_info));
  assert!(!error_info.is_null());
  let info = unsafe { &*error_info };
  assert_eq!(info.error_code, napi_ok as napi_status);

  // Now intentionally cause an error: call napi_get_value_double on a
  // non-number value (a boolean).
  let mut bool_val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut bool_val));

  let mut double_out: f64 = 0.0;
  let status = unsafe { napi_get_value_double(env, bool_val, &mut double_out) };
  assert_ne!(status, napi_ok as napi_status);

  // Retrieve the error info for the failed call.
  let mut error_info2: *const napi_extended_error_info = ptr::null();
  assert_napi_ok!(napi_get_last_error_info(env, &mut error_info2));
  assert!(!error_info2.is_null());
  let info2 = unsafe { &*error_info2 };
  assert_eq!(info2.error_code, status);

  // Return true to indicate all checks passed.
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test node_api_get_module_file_name.
extern "C" fn test_get_module_file_name(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut filename_ptr: *const std::os::raw::c_char = ptr::null();
  assert_napi_ok!(node_api_get_module_file_name(env, &mut filename_ptr));
  assert!(!filename_ptr.is_null());

  let filename = unsafe { CStr::from_ptr(filename_ptr) };
  let filename_str = filename.to_str().expect("invalid UTF-8 in module name");

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    filename_str.as_ptr() as *const std::os::raw::c_char,
    filename_str.len(),
    &mut result
  ));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_instanceof", test_instanceof),
    napi_new_property!(env, "test_get_version", test_get_version),
    napi_new_property!(env, "test_run_script", test_run_script),
    napi_new_property!(env, "test_get_node_version", test_get_node_version),
    napi_new_property!(
      env,
      "test_get_last_error_info",
      test_get_last_error_info
    ),
    napi_new_property!(
      env,
      "test_get_module_file_name",
      test_get_module_file_name
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
