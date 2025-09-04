// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use Status::napi_pending_exception;
use napi_sys::ValueType::napi_function;
use napi_sys::ValueType::napi_object;
use napi_sys::ValueType::napi_undefined;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

/// `test_callback_run((a, b) => a + b, [1, 2])` => 3
extern "C" fn test_callback_run(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  // We want to have argv with size 4, even though the callback will have
  // only two arguments. We'll assert that the remaining two args are undefined.
  let (args, argc, _) = napi_get_callback_info!(env, info, 4);
  assert_eq!(argc, 2);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_function);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[1], &mut ty));
  assert_eq!(ty, napi_object);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[2], &mut ty));
  assert_eq!(ty, napi_undefined);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[3], &mut ty));
  assert_eq!(ty, napi_undefined);

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

extern "C" fn test_callback_throws(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, ..) = napi_get_callback_info!(env, info, 1);

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut argv = vec![];
  let mut result: napi_value = ptr::null_mut();
  assert_eq!(
    unsafe {
      napi_call_function(
        env,
        global,  // recv
        args[0], // cb
        argv.len(),
        argv.as_mut_ptr(),
        &mut result,
      )
    },
    napi_pending_exception
  );

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
    napi_new_property!(env, "test_callback_throws", test_callback_throws),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
