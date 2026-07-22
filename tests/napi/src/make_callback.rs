// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::ValueType::napi_function;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::cstr;

extern "C" fn make_callback(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  const MAX_ARGUMENTS: usize = 10;
  const RESERVED_ARGUMENTS: usize = 3;

  let mut args = [std::ptr::null_mut(); MAX_ARGUMENTS];
  let mut argc = MAX_ARGUMENTS;
  assert_napi_ok!(napi_get_cb_info(
    env,
    info,
    &mut argc,
    args.as_mut_ptr(),
    ptr::null_mut(),
    ptr::null_mut(),
  ));

  assert!(argc > 0);
  let resource = args[0];
  let recv = args[1];
  let func = args[2];

  let mut argv: Vec<napi_value> = Vec::new();
  argv.resize(MAX_ARGUMENTS - RESERVED_ARGUMENTS, ptr::null_mut());
  for i in RESERVED_ARGUMENTS..argc {
    argv[i - RESERVED_ARGUMENTS] = args[i];
  }

  let mut func_type: napi_valuetype = -1;
  assert_napi_ok!(napi_typeof(env, func, &mut func_type));

  let mut resource_name = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("test"),
    usize::MAX,
    &mut resource_name
  ));

  let mut context: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(env, resource, resource_name, &mut context));

  let mut result = ptr::null_mut();
  assert_eq!(func_type, napi_function);
  assert_napi_ok!(napi_make_callback(
    env,
    context,
    recv,
    func,
    argc - RESERVED_ARGUMENTS,
    argv.as_mut_ptr(),
    &mut result
  ));

  assert_napi_ok!(napi_async_destroy(env, context));
  result
}

/// Recursive make_callback: calls a JS function via napi_make_callback,
/// passing itself as an argument so the JS side can call back into native
/// for a given depth. Tests that nested async contexts work correctly.
extern "C" fn make_callback_recurse(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let mut args = [ptr::null_mut(); 3];
  let mut argc = 3usize;
  assert_napi_ok!(napi_get_cb_info(
    env,
    info,
    &mut argc,
    args.as_mut_ptr(),
    ptr::null_mut(),
    ptr::null_mut(),
  ));

  // args: [resource, jsCallback, depth]
  let resource = args[0];
  let func = args[1];
  let depth_val = args[2];

  let mut depth: i32 = 0;
  assert_napi_ok!(napi_get_value_int32(env, depth_val, &mut depth));

  if depth <= 0 {
    // Base case: return depth (0)
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_create_int32(env, 0, &mut result));
    return result;
  }

  // Create async context
  let mut resource_name = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"recurse".as_ptr(),
    usize::MAX,
    &mut resource_name
  ));
  let mut context: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(env, resource, resource_name, &mut context));

  // Call JS function with (depth - 1) as argument
  let mut new_depth: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, depth - 1, &mut new_depth));

  let call_args = [new_depth];
  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_make_callback(
    env,
    context,
    resource,
    func,
    1,
    call_args.as_ptr(),
    &mut result
  ));

  assert_napi_ok!(napi_async_destroy(env, context));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let mut fn_: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_function(
    env,
    ptr::null_mut(),
    usize::MAX,
    Some(make_callback),
    ptr::null_mut(),
    &mut fn_,
  ));
  assert_napi_ok!(napi_set_named_property(
    env,
    exports,
    cstr!("makeCallback"),
    fn_
  ));

  let mut fn_recurse: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_function(
    env,
    ptr::null_mut(),
    usize::MAX,
    Some(make_callback_recurse),
    ptr::null_mut(),
    &mut fn_recurse,
  ));
  assert_napi_ok!(napi_set_named_property(
    env,
    exports,
    cstr!("makeCallbackRecurse"),
    fn_recurse
  ));
}
