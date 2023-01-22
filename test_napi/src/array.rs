// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_number;
use napi_sys::ValueType::napi_object;
use napi_sys::*;
use std::ptr;

extern "C" fn test_array_new(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_object);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_array(env, &mut value));

  let mut length: u32 = 0;
  assert_napi_ok!(napi_get_array_length(env, args[0], &mut length));

  for i in 0..length {
    let mut e: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_element(env, args[0], i, &mut e));
    assert_napi_ok!(napi_set_element(env, value, i, e));
  }

  value
}

extern "C" fn test_array_new_with_length(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_number);

  let mut len: u32 = 0;
  assert_napi_ok!(napi_get_value_uint32(env, args[0], &mut len));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_array_with_length(env, len as usize, &mut value));

  value
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_array_new", test_array_new),
    napi_new_property!(
      env,
      "test_array_new_with_length",
      test_array_new_with_length
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
