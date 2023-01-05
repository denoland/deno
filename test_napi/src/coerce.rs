// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::*;
use std::ptr;

extern "C" fn test_coerce_bool(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert!(unsafe { napi_coerce_to_bool(env, args[0], &mut value) } == napi_ok);
  value
}

extern "C" fn test_coerce_number(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert!(
    unsafe { napi_coerce_to_number(env, args[0], &mut value) } == napi_ok
  );
  value
}

extern "C" fn test_coerce_object(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert!(
    unsafe { napi_coerce_to_object(env, args[0], &mut value) } == napi_ok
  );
  value
}

extern "C" fn test_coerce_string(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value: napi_value = ptr::null_mut();
  assert!(
    unsafe { napi_coerce_to_string(env, args[0], &mut value) } == napi_ok
  );
  value
}
pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::new_property!(env, "test_coerce_bool\0", test_coerce_bool),
    crate::new_property!(env, "test_coerce_number\0", test_coerce_number),
    crate::new_property!(env, "test_coerce_object\0", test_coerce_object),
    crate::new_property!(env, "test_coerce_string\0", test_coerce_string),
  ];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
