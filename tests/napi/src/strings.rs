// Copyright 2018-2026 the Deno authors. MIT license.

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
    copied as isize,
    &mut result
  ));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_utf8", test_utf8),
    napi_new_property!(env, "test_utf16", test_utf16),
    napi_new_property!(env, "test_utf8_roundtrip", test_utf8_roundtrip),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
