// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_object;
use napi_sys::*;
use std::ptr;

unsafe extern "C" fn finalize_cb(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  assert!(data.is_null());
  assert!(hint.is_null());
}

extern "C" fn test_bind_finalizer(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_object);

  let obj = args[0];
  unsafe {
    napi_add_finalizer(
      env,
      obj,
      ptr::null_mut(),
      Some(finalize_cb),
      ptr::null_mut(),
      ptr::null_mut(),
    )
  };
  obj
}

struct Thing {
  _allocation: Vec<u8>,
}

impl Drop for Thing {
  fn drop(&mut self) {
    println!("Dropping Thing");
  }
}

unsafe extern "C" fn finalize_cb_drop(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  let _ = Box::from_raw(data as *mut Thing);
  assert!(hint.is_null());
}

extern "C" fn test_external_finalizer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let data = Box::into_raw(Box::new(Thing {
    _allocation: vec![1, 2, 3],
  }));

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    data as _,
    Some(finalize_cb_drop),
    ptr::null_mut(),
    &mut result
  ));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_bind_finalizer", test_bind_finalizer),
    napi_new_property!(env, "test_external_finalizer", test_external_finalizer),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
