// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;
use std::ptr;

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
pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_coerce_bool", test_coerce_bool),
    napi_new_property!(env, "test_coerce_number", test_coerce_number),
    napi_new_property!(env, "test_coerce_object", test_coerce_object),
    napi_new_property!(env, "test_coerce_string", test_coerce_string),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
