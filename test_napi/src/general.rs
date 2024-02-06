// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::cstr;
use crate::napi_new_property;
use napi_sys::Status::napi_ok;
use napi_sys::*;
use std::ptr;

extern "C" fn create_napi_error(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("xyz"),
    usize::MAX,
    &mut value
  ));

  let double_value = ptr::null_mut();
  let status = unsafe { napi_get_value_double(env, value, double_value) };
  assert_ne!(status, napi_ok);

  let mut error_info = ptr::null();
  let error_info_ptr = &mut error_info;
  assert_napi_ok!(napi_get_last_error_info(env, error_info_ptr));

  let err_info = unsafe { **error_info_ptr };
  assert_eq!(err_info.error_code, status);
  assert!(!err_info.error_message.is_null());
  ptr::null_mut()
}

extern "C" fn test_napi_error_cleanup(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut error_info = ptr::null();
  let error_info_ptr = &mut error_info;
  assert_napi_ok!(napi_get_last_error_info(env, error_info_ptr));

  let err_info = unsafe { **error_info_ptr };
  eprintln!("err_info {:#?}", err_info.error_code);
  let mut result: napi_value = ptr::null_mut();
  let is_ok = err_info.error_code == napi_ok;
  assert_napi_ok!(napi_get_boolean(env, is_ok, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "createNapiError", create_napi_error),
    napi_new_property!(env, "testNapiErrorCleanup", test_napi_error_cleanup),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
