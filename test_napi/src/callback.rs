// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_function;
use napi_sys::ValueType::napi_object;
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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_callback_run", test_callback_run),
    napi_new_property!(
      env,
      "test_callback_run_with_recv",
      test_callback_run_with_recv
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
