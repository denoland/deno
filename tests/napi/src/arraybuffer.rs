// Copyright 2018-2025 the Deno authors. MIT license.

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn test_detached(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value = false;
  assert_napi_ok!(napi_is_detached_arraybuffer(env, args[0], &mut value));
  assert!(!value);
  assert_napi_ok!(napi_detach_arraybuffer(env, args[0]));
  assert_napi_ok!(napi_is_detached_arraybuffer(env, args[0], &mut value));
  assert!(value);
  args[0]
}

extern "C" fn is_detached(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value = false;
  assert_napi_ok!(napi_is_detached_arraybuffer(env, args[0], &mut value));

  let mut result = std::ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, value, &mut result));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_detached", test_detached),
    napi_new_property!(env, "is_detached", is_detached),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
