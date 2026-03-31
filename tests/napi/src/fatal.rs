// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

/// Calls napi_fatal_error with a location and message.
/// This will abort the process — only call from a subprocess test.
extern "C" fn test_fatal_error(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);

  if argc >= 2 {
    // Get location string
    let mut location_buf = [0u8; 256];
    let mut location_len: usize = 0;
    assert_napi_ok!(napi_get_value_string_utf8(
      env,
      args[0],
      location_buf.as_mut_ptr() as *mut std::os::raw::c_char,
      location_buf.len(),
      &mut location_len,
    ));

    // Get message string
    let mut message_buf = [0u8; 256];
    let mut message_len: usize = 0;
    assert_napi_ok!(napi_get_value_string_utf8(
      env,
      args[1],
      message_buf.as_mut_ptr() as *mut std::os::raw::c_char,
      message_buf.len(),
      &mut message_len,
    ));

    unsafe {
      napi_fatal_error(
        location_buf.as_ptr() as *const std::os::raw::c_char,
        location_len,
        message_buf.as_ptr() as *const std::os::raw::c_char,
        message_len,
      );
    }
  } else {
    // No args: call with NAPI_AUTO_LENGTH (null-terminated strings)
    unsafe {
      napi_fatal_error(
        c"test_location".as_ptr(),
        usize::MAX, // NAPI_AUTO_LENGTH
        c"test fatal message".as_ptr(),
        usize::MAX, // NAPI_AUTO_LENGTH
      );
    }
  }

  // Unreachable — napi_fatal_error aborts the process
  ptr::null_mut()
}

/// Calls napi_fatal_exception with an Error object.
/// This triggers the uncaught exception handler.
extern "C" fn test_fatal_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  assert_napi_ok!(napi_fatal_exception(env, args[0]));

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_fatal_error", test_fatal_error),
    napi_new_property!(env, "test_fatal_exception", test_fatal_exception),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
