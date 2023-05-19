// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;
use std::ffi::c_void;
use std::ptr;

#[repr(C)]
#[derive(Debug)]
struct AddonData {
  value: u32,
  print: bool,
  js_cb_ref: napi_ref,
}

extern "C" fn delete_addon_data(
  env: napi_env,
  data: *mut c_void,
  _hint: *mut c_void,
) {
  let data: Box<AddonData> = unsafe { Box::from_raw(data as *mut AddonData) };
  if data.print {
    println!("deleting addon data");
  }
  if !data.js_cb_ref.is_null() {
    assert_napi_ok!(napi_delete_reference(env, data.js_cb_ref));
  }
}

extern "C" fn increment(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let data: *mut AddonData = ptr::null_mut();
  let data_ptr = &mut (data as *mut c_void);
  let mut result: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_get_instance_data(env, data_ptr));
  let data = unsafe { &mut *(*data_ptr as *mut AddonData) };
  eprintln!("data {:#?}", data);
  assert_napi_ok!(napi_create_uint32(env, data.value + 1, &mut result));

  result
}

extern "C" fn set_print_on_delete(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let data: *mut AddonData = ptr::null_mut();
  let data_ptr = &mut (data as *mut c_void);
  assert_napi_ok!(napi_get_instance_data(env, data_ptr));
  unsafe { (*(*data_ptr as *mut AddonData)).print = true };

  ptr::null_mut()
}

unsafe extern "C" fn test_finalizer(
  env: napi_env,
  _raw_data: *mut c_void,
  _hint: *mut c_void,
) {
  let data: *mut AddonData = ptr::null_mut();
  let data_ptr = &mut (data as *mut c_void);
  assert_napi_ok!(napi_get_instance_data(env, data_ptr));
  let data = unsafe { &mut *(*data_ptr as *mut AddonData) };
  eprintln!("data {:#?}", data);
  let mut js_cb: napi_value = ptr::null_mut();
  let mut undefined: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_reference_value(env, data.js_cb_ref, &mut js_cb));
  eprintln!("js_cb {:#?}", js_cb);
  assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  assert_napi_ok!(napi_call_function(
    env,
    undefined,
    js_cb,
    0,
    ptr::null_mut(),
    ptr::null_mut()
  ));
  assert_napi_ok!(napi_delete_reference(env, data.js_cb_ref));
  data.js_cb_ref = ptr::null_mut();
}

extern "C" fn object_with_finalizer(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let data: *mut AddonData = ptr::null_mut();
  let data_ptr = &mut (data as *mut c_void);
  let mut result: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_get_instance_data(env, data_ptr));
  let data = unsafe { (&mut *(*data_ptr as *mut AddonData)) };
  assert!(data.js_cb_ref.is_null());

  let (_args, argc, js_cb) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut result));
  assert_napi_ok!(napi_add_finalizer(
    env,
    result,
    ptr::null_mut(),
    Some(test_finalizer),
    ptr::null_mut(),
    ptr::null_mut(),
  ));
  assert_napi_ok!(napi_create_reference(env, js_cb, 1, &mut data.js_cb_ref));

  value
}

pub fn init(env: napi_env, exports: napi_value) {
  let data = Box::new(AddonData {
    value: 41,
    print: false,
    js_cb_ref: ptr::null_mut(),
  });
  let raw_data = Box::into_raw(data);
  assert_napi_ok!(napi_set_instance_data(
    env,
    raw_data as *mut c_void,
    Some(delete_addon_data),
    ptr::null_mut(),
  ));

  let properties = &[
    napi_new_property!(env, "increment", increment),
    napi_new_property!(env, "setPrintOnDelete", set_print_on_delete),
    napi_new_property!(env, "objectWithFinalizer", object_with_finalizer),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
