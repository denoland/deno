// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_instanceof", test_instanceof),
    napi_new_property!(env, "test_get_version", test_get_version),
    napi_new_property!(env, "test_run_script", test_run_script),
    napi_new_property!(env, "test_get_node_version", test_get_node_version),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
