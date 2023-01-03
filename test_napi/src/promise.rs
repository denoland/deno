// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::*;
use std::ptr;

static mut CURRENT_DEFERRED: napi_deferred = ptr::null_mut();

extern "C" fn test_promise_new(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut value: napi_value = ptr::null_mut();
  assert!(
    unsafe { napi_create_promise(env, &mut CURRENT_DEFERRED, &mut value) }
      == napi_ok
  );
  value
}

extern "C" fn test_promise_resolve(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  assert!(
    unsafe { napi_resolve_deferred(env, CURRENT_DEFERRED, args[0]) } == napi_ok
  );
  unsafe { CURRENT_DEFERRED = ptr::null_mut() };
  ptr::null_mut()
}

extern "C" fn test_promise_reject(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  assert!(
    unsafe { napi_reject_deferred(env, CURRENT_DEFERRED, args[0]) } == napi_ok
  );
  unsafe { CURRENT_DEFERRED = ptr::null_mut() };
  ptr::null_mut()
}

extern "C" fn test_promise_is(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut is_promise: bool = false;
  assert!(unsafe { napi_is_promise(env, args[0], &mut is_promise) } == napi_ok);

  let mut result: napi_value = ptr::null_mut();
  assert!(unsafe { napi_get_boolean(env, is_promise, &mut result) } == napi_ok);

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::new_property!(env, "test_promise_new\0", test_promise_new),
    crate::new_property!(env, "test_promise_resolve\0", test_promise_resolve),
    crate::new_property!(env, "test_promise_reject\0", test_promise_reject),
    crate::new_property!(env, "test_promise_is\0", test_promise_is),
  ];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
