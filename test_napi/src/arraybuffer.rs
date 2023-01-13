// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;

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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(env, "test_detached", test_detached)];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
