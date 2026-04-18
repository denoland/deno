// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_char;

use napi_sys::ValueType::napi_string;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn test_utf8(env: napi_env, info: napi_callback_info) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_string);

  args[0]
}

extern "C" fn test_utf16(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_string);

  args[0]
}

extern "C" fn test_utf8_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  let mut buf: Vec<u8> = vec![0; 1024];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    buf.as_mut_ptr() as *mut std::ffi::c_char,
    buf.len(),
    &mut copied
  ));

  assert_eq!(copied, len);

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    buf.as_ptr() as *const std::ffi::c_char,
    copied,
    &mut result
  ));

  result
}

extern "C" fn test_property_key_latin1(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from latin1 string "hello"
  let latin1_str = b"hello\0";
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_latin1(
    env,
    latin1_str.as_ptr() as *const c_char,
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_property_key_utf8(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from utf8 string "hello"
  let utf8_str = b"hello\0";
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_utf8(
    env,
    utf8_str.as_ptr() as *const c_char,
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_property_key_utf16(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from utf16 string "hello"
  let utf16_str: [u16; 6] = [
    'h' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16, 0,
  ];
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_utf16(
    env,
    utf16_str.as_ptr(),
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_latin1_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  // Get length
  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_latin1(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  // Get string content
  let mut buf: Vec<u8> = vec![0; len + 1];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_latin1(
    env,
    args[0],
    buf.as_mut_ptr() as *mut c_char,
    buf.len(),
    &mut copied
  ));
  assert_eq!(copied, len);

  // Create string from latin1 bytes
  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_latin1(
    env,
    buf.as_ptr() as *const c_char,
    copied,
    &mut result
  ));

  result
}

extern "C" fn test_utf16_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  // Get length
  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf16(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  // Get string content
  let mut buf: Vec<u16> = vec![0; len + 1];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf16(
    env,
    args[0],
    buf.as_mut_ptr(),
    buf.len(),
    &mut copied
  ));
  assert_eq!(copied, len);

  // Create string from utf16
  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf16(
    env,
    buf.as_ptr(),
    copied,
    &mut result
  ));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_utf8", test_utf8),
    napi_new_property!(env, "test_utf16", test_utf16),
    napi_new_property!(env, "test_utf8_roundtrip", test_utf8_roundtrip),
    napi_new_property!(
      env,
      "test_property_key_latin1",
      test_property_key_latin1
    ),
    napi_new_property!(env, "test_property_key_utf8", test_property_key_utf8),
    napi_new_property!(env, "test_property_key_utf16", test_property_key_utf16),
    napi_new_property!(env, "test_latin1_roundtrip", test_latin1_roundtrip),
    napi_new_property!(env, "test_utf16_roundtrip", test_utf16_roundtrip),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
