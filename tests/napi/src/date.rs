// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::ValueType::napi_number;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn create_date(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_number);

  let mut time = -1.0;
  assert_napi_ok!(napi_get_value_double(env, args[0], &mut time));

  let mut date: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_date(env, time, &mut date));

  date
}

extern "C" fn is_date(env: napi_env, info: napi_callback_info) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let date: napi_value = args[0];
  let mut result: napi_value = std::ptr::null_mut();
  let mut is_date = false;

  assert_napi_ok!(napi_is_date(env, date, &mut is_date));
  assert_napi_ok!(napi_get_boolean(env, is_date, &mut result));

  result
}

extern "C" fn get_date_value(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let date: napi_value = args[0];
  let mut result: napi_value = std::ptr::null_mut();
  let mut value = 0.0;

  assert_napi_ok!(napi_get_date_value(env, date, &mut value));
  assert_napi_ok!(napi_create_double(env, value, &mut result));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "createDate", create_date),
    napi_new_property!(env, "isDate", is_date),
    napi_new_property!(env, "getDateValue", get_date_value),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
