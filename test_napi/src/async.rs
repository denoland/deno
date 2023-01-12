// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_function;
use napi_sys::*;
use std::os::raw::{c_char, c_void};
use std::ptr;

pub struct Baton {
  called: bool,
  func: napi_ref,
  task: napi_async_work,
}

unsafe extern "C" fn execute(_env: napi_env, data: *mut c_void) {
  let baton: &mut Baton = &mut *(data as *mut Baton);
  assert!(!baton.called);
  assert!(!baton.func.is_null());

  baton.called = true;
}

unsafe extern "C" fn complete(
  env: napi_env,
  status: napi_status,
  data: *mut c_void,
) {
  assert!(status == napi_ok);
  let baton: Box<Baton> = Box::from_raw(data as *mut Baton);
  assert!(baton.called);
  assert!(!baton.func.is_null());

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut callback: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_reference_value(env, baton.func, &mut callback));

  let mut _result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_call_function(
    env,
    global,
    callback,
    0,
    ptr::null(),
    &mut _result
  ));

  assert_napi_ok!(napi_delete_reference(env, baton.func));
  assert_napi_ok!(napi_delete_async_work(env, baton.task));
}

extern "C" fn test_async_work(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_function);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    "test_async_resource".as_ptr() as *const c_char,
    usize::MAX,
    &mut resource_name,
  ));

  let mut async_work: napi_async_work = ptr::null_mut();

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));
  let baton = Box::new(Baton {
    called: false,
    func,
    task: async_work,
  });

  assert_napi_ok!(napi_create_async_work(
    env,
    ptr::null_mut(),
    resource_name,
    Some(execute),
    Some(complete),
    Box::into_raw(baton) as *mut c_void,
    &mut async_work,
  ));
  assert_napi_ok!(napi_queue_async_work(env, async_work));

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties =
    &[napi_new_property!(env, "test_async_work", test_async_work)];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
