// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::Status::napi_pending_exception;
use napi_sys::*;
use std::ptr;

static mut EXCEPTION_WAS_PENDING: bool = false;

extern "C" fn return_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  // FIXME:
  let status = unsafe {
    napi_call_function(env, global, args[0], 0, ptr::null(), &mut result)
  };
  eprintln!("status {:#?}", status);
  if status == napi_pending_exception {
    let mut ex: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_and_clear_last_exception(env, &mut ex));
    return ex;
  }
  ptr::null_mut()
}

extern "C" fn allow_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  unsafe {
    napi_call_function(env, global, args[0], 0, ptr::null(), &mut result)
  };

  assert_napi_ok!(napi_is_exception_pending(env, &mut EXCEPTION_WAS_PENDING));
  ptr::null_mut()
}

extern "C" fn construct_return_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut result: napi_value = ptr::null_mut();
  let status =
    unsafe { napi_new_instance(env, args[0], 0, ptr::null(), &mut result) };
  if status == napi_pending_exception {
    let mut ex: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_and_clear_last_exception(env, &mut ex));
    return ex;
  }
  ptr::null_mut()
}

extern "C" fn construct_allow_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut result: napi_value = ptr::null_mut();
  unsafe { napi_new_instance(env, args[0], 0, ptr::null(), &mut result) };

  assert_napi_ok!(napi_is_exception_pending(env, &mut EXCEPTION_WAS_PENDING));
  ptr::null_mut()
}

extern "C" fn was_pending(env: napi_env, _: napi_callback_info) -> napi_value {
  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, EXCEPTION_WAS_PENDING, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "returnException", return_exception),
    napi_new_property!(env, "allowException", allow_exception),
    napi_new_property!(
      env,
      "constructReturnException",
      construct_return_exception
    ),
    napi_new_property!(
      env,
      "constructAllowException",
      construct_allow_exception
    ),
    napi_new_property!(env, "wasPending", was_pending),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
