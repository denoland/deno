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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(
    env,
    "test_bind_finalizer",
    test_bind_finalizer
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
