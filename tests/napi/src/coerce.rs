// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::Status::napi_ok;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn test_coerce_bool(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_coerce_to_bool(env, args[0], &mut value));
  value
}

extern "C" fn test_coerce_number(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_coerce_to_number(env, args[0], &mut value));
  value
}

extern "C" fn test_coerce_object(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_coerce_to_object(env, args[0], &mut value));
  value
}

extern "C" fn test_coerce_string(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_coerce_to_string(env, args[0], &mut value));
  value
}

/// Calls napi_coerce_to_object on null and undefined, verifying that:
/// 1. It returns an error status (not napi_ok)
/// 2. No pending exception is left behind
extern "C" fn test_coerce_object_null_no_pending_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  // args[0] should be null, args[1] should be undefined
  for i in 0..2 {
    let mut result: napi_value = ptr::null_mut();
    let status = unsafe { napi_coerce_to_object(env, args[i], &mut result) };
    // Should fail
    assert_ne!(status, napi_ok);

    // Verify no pending exception was left behind
    let mut is_pending = false;
    assert_napi_ok!(napi_is_exception_pending(env, &mut is_pending));
    assert!(
      !is_pending,
      "napi_coerce_to_object should not leave a pending exception"
    );
  }

  let mut ret: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut ret));
  ret
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_coerce_bool", test_coerce_bool),
    napi_new_property!(env, "test_coerce_number", test_coerce_number),
    napi_new_property!(env, "test_coerce_object", test_coerce_object),
    napi_new_property!(env, "test_coerce_string", test_coerce_string),
    napi_new_property!(
      env,
      "test_coerce_object_null_no_pending_exception",
      test_coerce_object_null_no_pending_exception
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
