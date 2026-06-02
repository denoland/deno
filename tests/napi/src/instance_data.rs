// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_void;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

unsafe extern "C" fn instance_data_finalizer(
  _env: napi_env,
  data: *mut c_void,
  _hint: *mut c_void,
) {
  println!("instance_data_free({})", data as i64);
}

extern "C" fn set_instance_data(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 0);

  let data = 42 as *mut c_void;
  assert_napi_ok!(napi_set_instance_data(
    env,
    data,
    Some(instance_data_finalizer),
    std::ptr::null_mut(),
  ));

  std::ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(
    env,
    "setInstanceData",
    set_instance_data
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
