// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_string;
use napi_sys::*;
use std::ptr;

extern "C" fn test_utf8(env: napi_env, info: napi_callback_info) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_string);

  args[0]
}

extern "C" fn test_utf16(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_string);

  args[0]
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    // utf8
    crate::new_property!(env, "test_utf8\0", test_utf8),
    // utf16
    crate::new_property!(env, "test_utf16\0", test_utf16),
    // latin1
  ];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
