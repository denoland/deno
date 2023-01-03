// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_number;
use napi_sys::ValueType::napi_object;
use napi_sys::*;
use std::ptr;

extern "C" fn test_array_new(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_object);

  let mut value: napi_value = ptr::null_mut();
  assert!(unsafe { napi_create_array(env, &mut value) } == napi_ok);

  let mut length: u32 = 0;
  assert!(
    unsafe { napi_get_array_length(env, args[0], &mut length) } == napi_ok
  );

  for i in 0..length {
    let mut e: napi_value = ptr::null_mut();
    assert!(unsafe { napi_get_element(env, args[0], i, &mut e) } == napi_ok);
    assert!(unsafe { napi_set_element(env, value, i, e) } == napi_ok);
  }

  value
}

extern "C" fn test_array_new_with_length(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_number);

  let mut len: u32 = 0;
  assert!(unsafe { napi_get_value_uint32(env, args[0], &mut len) } == napi_ok);

  let mut value: napi_value = ptr::null_mut();
  assert!(
    unsafe { napi_create_array_with_length(env, len as usize, &mut value) }
      == napi_ok
  );

  value
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::new_property!(env, "test_array_new\0", test_array_new),
    crate::new_property!(
      env,
      "test_array_new_with_length\0",
      test_array_new_with_length
    ),
  ];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
