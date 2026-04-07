// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

/// Test napi_is_exception_pending: throw an error, check pending, then clear.
extern "C" fn test_exception_pending(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // No exception should be pending initially
  let mut is_pending = true;
  assert_napi_ok!(napi_is_exception_pending(env, &mut is_pending));
  assert!(!is_pending);

  // Throw an error
  unsafe {
    napi_throw_error(env, ptr::null(), c"test error".as_ptr());
  }

  // Now an exception should be pending
  unsafe {
    napi_is_exception_pending(env, &mut is_pending);
  }
  assert!(is_pending);

  // Clear the exception
  let mut exception: napi_value = ptr::null_mut();
  unsafe {
    napi_get_and_clear_last_exception(env, &mut exception);
  }

  // Should no longer be pending
  assert_napi_ok!(napi_is_exception_pending(env, &mut is_pending));
  assert!(!is_pending);

  // Return true to indicate success
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test napi_get_and_clear_last_exception returns the thrown value.
extern "C" fn test_get_clear_exception(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Throw a string value as exception
  let mut error_msg: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"my error message".as_ptr(),
    16,
    &mut error_msg
  ));
  unsafe {
    napi_throw(env, error_msg);
  }

  // Clear and retrieve the exception
  let mut exception: napi_value = ptr::null_mut();
  unsafe {
    napi_get_and_clear_last_exception(env, &mut exception);
  }

  // The exception should be the string we threw
  exception
}

/// Test exception propagation through napi_call_function.
/// Calls a JS function that throws, verifies exception is pending,
/// then clears it and returns the error message.
extern "C" fn test_exception_from_call(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let func = args[0];

  // Call the function (it will throw)
  let mut result: napi_value = ptr::null_mut();
  let mut global: napi_value = ptr::null_mut();
  unsafe {
    napi_get_global(env, &mut global);
    napi_call_function(env, global, func, 0, ptr::null(), &mut result);
  }

  // Check exception is pending
  let mut is_pending = false;
  unsafe {
    napi_is_exception_pending(env, &mut is_pending);
  }
  assert!(is_pending);

  // Get and clear the exception
  let mut exception: napi_value = ptr::null_mut();
  unsafe {
    napi_get_and_clear_last_exception(env, &mut exception);
  }

  // Extract the message property from the Error object
  let mut msg_key: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"message".as_ptr(),
    7,
    &mut msg_key
  ));
  let mut msg_val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, exception, msg_key, &mut msg_val));

  msg_val
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_exception_pending", test_exception_pending),
    napi_new_property!(
      env,
      "test_get_clear_exception",
      test_get_clear_exception
    ),
    napi_new_property!(
      env,
      "test_exception_from_call",
      test_exception_from_call
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
