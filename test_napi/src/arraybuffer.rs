// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::*;
use std::ptr;

extern "C" fn test_detached(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut value = false;
  assert!(
    unsafe { napi_is_detached_arraybuffer(env, args[0], &mut value) }
      == napi_ok
  );
  assert!(!value);
  assert!(unsafe { napi_detach_arraybuffer(env, args[0]) } == napi_ok);
  assert!(
    unsafe { napi_is_detached_arraybuffer(env, args[0], &mut value) }
      == napi_ok
  );
  assert!(value);
  args[0]
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties =
    &[crate::new_property!(env, "test_detached\0", test_detached)];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
