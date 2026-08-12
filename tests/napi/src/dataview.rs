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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_dataview", test_dataview),
    napi_new_property!(env, "test_is_dataview", test_is_dataview),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
