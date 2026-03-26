// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

/// Test napi_create_buffer: creates a Buffer of given size.
extern "C" fn test_create_buffer(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut data: *mut std::ffi::c_void = ptr::null_mut();
  let mut result: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_buffer(env, 10, &mut data, &mut result));

  // Write some data into the buffer
  unsafe {
    let slice = std::slice::from_raw_parts_mut(data as *mut u8, 10);
    for (i, byte) in slice.iter_mut().enumerate() {
      *byte = i as u8;
    }
  }

  result
}

/// Test napi_create_buffer_copy: creates a Buffer by copying data.
extern "C" fn test_create_buffer_copy(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let source: [u8; 5] = [10, 20, 30, 40, 50];
  let mut result_data: *mut std::ffi::c_void = ptr::null_mut();
  let mut result: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_buffer_copy(
    env,
    source.len(),
    source.as_ptr() as *const std::ffi::c_void,
    &mut result_data,
    &mut result
  ));

  result
}

/// Test napi_get_buffer_info: retrieves data pointer and length.
extern "C" fn test_get_buffer_info(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Create a buffer with known content
  let source: [u8; 3] = [0xAA, 0xBB, 0xCC];
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_buffer_copy(
    env,
    source.len(),
    source.as_ptr() as *const std::ffi::c_void,
    ptr::null_mut(),
    &mut result
  ));

  // Get buffer info
  let mut data: *mut std::ffi::c_void = ptr::null_mut();
  let mut length: usize = 0;
  assert_napi_ok!(napi_get_buffer_info(env, result, &mut data, &mut length));

  assert_eq!(length, 3);
  let slice = unsafe { std::slice::from_raw_parts(data as *const u8, length) };
  assert_eq!(slice, &[0xAA, 0xBB, 0xCC]);

  // Return the length as confirmation
  let mut len_val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, length as i32, &mut len_val));
  len_val
}

/// Test napi_is_buffer on a Buffer vs a non-Buffer.
extern "C" fn test_is_buffer(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Create a buffer
  let mut buf: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_buffer(env, 5, ptr::null_mut(), &mut buf));

  let mut is_buf = false;
  assert_napi_ok!(napi_is_buffer(env, buf, &mut is_buf));
  assert!(is_buf);

  // Create a plain object (not a buffer)
  let mut obj: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));
  assert_napi_ok!(napi_is_buffer(env, obj, &mut is_buf));
  assert!(!is_buf);

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_create_buffer", test_create_buffer),
    napi_new_property!(env, "test_create_buffer_copy", test_create_buffer_copy),
    napi_new_property!(env, "test_get_buffer_info", test_get_buffer_info),
    napi_new_property!(env, "test_is_buffer_check", test_is_buffer),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
