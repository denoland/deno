// Copyright 2018-2025 the Deno authors. MIT license.

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
}
