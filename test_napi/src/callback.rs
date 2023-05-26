// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_boolean;
use napi_sys::ValueType::napi_function;
use napi_sys::ValueType::napi_number;
use napi_sys::ValueType::napi_object;
use napi_sys::ValueType::napi_string;
use napi_sys::ValueType::napi_undefined;
use napi_sys::*;
use std::ptr;

/// `test_callback_run((a, b) => a + b, [1, 2])` => 3
extern "C" fn test_callback_run(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_function);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[1], &mut ty));
  assert_eq!(ty, napi_object);

  let mut len = 0;
  assert_napi_ok!(napi_get_array_length(env, args[1], &mut len));

  let mut argv = Vec::with_capacity(len as usize);
  for index in 0..len {
    let mut value: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_element(env, args[1], index, &mut value));
    argv.push(value);
  }
  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_call_function(
    env,
    global,
    args[0],
    argv.len(),
    argv.as_mut_ptr(),
    &mut result,
  ));

  result
}

extern "C" fn test_callback_run_with_recv(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 3);
  assert_eq!(argc, 3);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_function);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[1], &mut ty));
  assert_eq!(ty, napi_object);

  let mut len = 0;
  assert_napi_ok!(napi_get_array_length(env, args[1], &mut len));

  let mut argv = Vec::with_capacity(len as usize);
  for index in 0..len {
    let mut value: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_element(env, args[1], index, &mut value));
    argv.push(value);
  }

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_call_function(
    env,
    args[2], // recv
    args[0], // cb
    argv.len(),
    argv.as_mut_ptr(),
    &mut result,
  ));

  result
}

extern "C" fn test_callback_with_optional_args(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let mut argc = 0;
  let mut args = [std::ptr::null_mut(); 10];
  let mut this: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_get_cb_info(
    env,
    info,
    &mut argc,
    args.as_mut_ptr(),
    &mut this,
    std::ptr::null_mut(),
  ));

  for i in 0..4 {
    let mut ty = -1;
    assert_napi_ok!(napi_typeof(env, args[i], &mut ty));
    assert_eq!(
      ty,
      match i {
        0 => napi_boolean,
        1 => napi_number,
        2 => napi_string,
        3 => napi_undefined,
        _ => unreachable!(),
      }
    );
  }

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_callback_run", test_callback_run),
    napi_new_property!(
      env,
      "test_callback_run_with_recv",
      test_callback_run_with_recv
    ),
    napi_new_property!(
      env,
      "test_callback_with_optional_args",
      test_callback_with_optional_args
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
