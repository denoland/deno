// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_string;
use napi_sys::*;

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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    // utf8
    napi_new_property!(env, "test_utf8", test_utf8),
    // utf16
    napi_new_property!(env, "test_utf16", test_utf16),
    // latin1
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
