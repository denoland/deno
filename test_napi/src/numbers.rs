// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_number;
use napi_sys::*;
use std::ptr;

extern "C" fn test_int32(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_number);

  let mut int32 = -1;
  assert!(unsafe { napi_get_value_int32(env, args[0], &mut int32) } == napi_ok);

  let mut value: napi_value = ptr::null_mut();
  assert!(unsafe { napi_create_int32(env, int32, &mut value) } == napi_ok);
  value
}

extern "C" fn test_int64(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_number);

  let mut int64 = -1;
  assert!(unsafe { napi_get_value_int64(env, args[0], &mut int64) } == napi_ok);

  let mut value: napi_value = ptr::null_mut();
  assert!(unsafe { napi_create_int64(env, int64, &mut value) } == napi_ok);
  value
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::new_property!(env, "test_int32\0", test_int32),
    crate::new_property!(env, "test_int64\0", test_int64),
  ];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
