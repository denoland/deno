// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;
use std::ptr;

extern "C" fn test_object_new(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut value));

  assert_napi_ok!(napi_set_element(env, value, 0, args[0]));
  assert_napi_ok!(napi_set_element(env, value, 1, args[1]));

  value
}

extern "C" fn test_object_get(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let obj = args[0];
  assert_napi_ok!(napi_set_element(env, obj, 0, args[0]));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_element(env, obj, 0, &mut value));
  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_element(env, obj, 1, &mut value));

  obj
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_object_new", test_object_new),
    napi_new_property!(env, "test_object_get", test_object_get),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
