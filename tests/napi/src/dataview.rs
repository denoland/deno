// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

/// Test napi_create_dataview and napi_get_dataview_info.
extern "C" fn test_dataview(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Create an ArrayBuffer first
  let mut ab: napi_value = ptr::null_mut();
  let mut ab_data: *mut std::ffi::c_void = ptr::null_mut();
  assert_napi_ok!(napi_create_arraybuffer(env, 16, &mut ab_data, &mut ab));

  // Create a DataView over it with offset=4 and length=8
  let mut dv: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_dataview(env, 8, ab, 4, &mut dv));

  // Verify it is a DataView
  let mut is_dv = false;
  assert_napi_ok!(napi_is_dataview(env, dv, &mut is_dv));
  assert!(is_dv);

  // Get DataView info
  let mut byte_length: usize = 0;
  let mut data: *mut std::ffi::c_void = ptr::null_mut();
  let mut arraybuffer: napi_value = ptr::null_mut();
  let mut byte_offset: usize = 0;
  assert_napi_ok!(napi_get_dataview_info(
    env,
    dv,
    &mut byte_length,
    &mut data,
    &mut arraybuffer,
    &mut byte_offset
  ));

  assert_eq!(byte_length, 8);
  assert_eq!(byte_offset, 4);
  assert!(!data.is_null());

  // Return the byte_length to confirm
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, byte_length as i32, &mut result));
  result
}

/// Test napi_is_dataview on non-DataView values.
extern "C" fn test_is_dataview(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // A plain object is not a DataView
  let mut obj: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));
  let mut is_dv = true;
  assert_napi_ok!(napi_is_dataview(env, obj, &mut is_dv));
  assert!(!is_dv);

  // A Uint8Array is not a DataView
  let mut ab: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_arraybuffer(env, 8, ptr::null_mut(), &mut ab));
  let mut ta: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_typedarray(
    env,
    TypedarrayType::uint8_array,
    8,
    ab,
    0,
    &mut ta
  ));
  assert_napi_ok!(napi_is_dataview(env, ta, &mut is_dv));
  assert!(!is_dv);

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Call napi_create_dataview with an out of range byte_length/byte_offset over
/// a 16 byte buffer: it throws a RangeError, returns napi_pending_exception
/// and leaves the result unset.
/// Returns [status, exception pending, result set] after clearing the error.
fn create_dataview_out_of_range(
  env: napi_env,
  byte_length: usize,
  byte_offset: usize,
) -> napi_value {
  let mut ab: napi_value = ptr::null_mut();
  let mut ab_data: *mut std::ffi::c_void = ptr::null_mut();
  assert_napi_ok!(napi_create_arraybuffer(env, 16, &mut ab_data, &mut ab));

  let mut dv: napi_value = ptr::null_mut();
  let status =
    unsafe { napi_create_dataview(env, byte_length, ab, byte_offset, &mut dv) };

  let mut is_pending = false;
  let mut exception: napi_value = ptr::null_mut();
  unsafe {
    napi_is_exception_pending(env, &mut is_pending);
    napi_get_and_clear_last_exception(env, &mut exception);
  }

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_array_with_length(env, 3, &mut result));
  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, status, &mut value));
  assert_napi_ok!(napi_set_element(env, result, 0, value));
  assert_napi_ok!(napi_get_boolean(env, is_pending, &mut value));
  assert_napi_ok!(napi_set_element(env, result, 1, value));
  assert_napi_ok!(napi_get_boolean(env, !dv.is_null(), &mut value));
  assert_napi_ok!(napi_set_element(env, result, 2, value));
  result
}

/// Test napi_create_dataview past the end of the buffer.
extern "C" fn test_dataview_out_of_range(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  create_dataview_out_of_range(env, 8, 12)
}

/// Test napi_create_dataview where byte_offset + byte_length overflows usize.
extern "C" fn test_dataview_overflow(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  create_dataview_out_of_range(env, usize::MAX, 12)
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_dataview", test_dataview),
    napi_new_property!(env, "test_is_dataview", test_is_dataview),
    napi_new_property!(
      env,
      "test_dataview_out_of_range",
      test_dataview_out_of_range
    ),
    napi_new_property!(env, "test_dataview_overflow", test_dataview_overflow),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
