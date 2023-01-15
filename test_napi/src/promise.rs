// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;
use std::ptr;

static mut CURRENT_DEFERRED: napi_deferred = ptr::null_mut();

extern "C" fn test_promise_new(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_promise(env, &mut CURRENT_DEFERRED, &mut value));
  value
}

extern "C" fn test_promise_resolve(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  assert_napi_ok!(napi_resolve_deferred(env, CURRENT_DEFERRED, args[0]));
  unsafe { CURRENT_DEFERRED = ptr::null_mut() };
  ptr::null_mut()
}

extern "C" fn test_promise_reject(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  assert_napi_ok!(napi_reject_deferred(env, CURRENT_DEFERRED, args[0]));
  unsafe { CURRENT_DEFERRED = ptr::null_mut() };
  ptr::null_mut()
}

extern "C" fn test_promise_is(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut is_promise: bool = false;
  assert_napi_ok!(napi_is_promise(env, args[0], &mut is_promise));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, is_promise, &mut result));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_promise_new", test_promise_new),
    napi_new_property!(env, "test_promise_resolve", test_promise_resolve),
    napi_new_property!(env, "test_promise_reject", test_promise_reject),
    napi_new_property!(env, "test_promise_is", test_promise_is),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
