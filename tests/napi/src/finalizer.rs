// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::ValueType::napi_object;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

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

unsafe extern "C" fn finalize_cb_drop(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  unsafe {
    let _ = Box::from_raw(data as *mut Thing);
    assert!(hint.is_null());
  }
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

unsafe extern "C" fn finalize_cb_vec(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  unsafe {
    let _ = Vec::from_raw_parts(data as *mut u8, 3, 3);
    assert!(hint.is_null());
  }
}

extern "C" fn test_external_buffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let buf: Vec<u8> = vec![1, 2, 3];
  assert_napi_ok!(napi_create_external_buffer(
    env,
    3,
    buf.as_ptr() as _,
    Some(finalize_cb_vec),
    ptr::null_mut(),
    &mut result
  ));
  std::mem::forget(buf);

  result
}

extern "C" fn test_static_external_buffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  static BUF: &[u8] = &[1, 2, 3];
  assert_napi_ok!(napi_create_external_buffer(
    env,
    BUF.len(),
    BUF.as_ptr() as _,
    None,
    ptr::null_mut(),
    &mut result
  ));

  result
}

extern "C" fn test_external_arraybuffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let buf: Vec<u8> = vec![1, 2, 3];
  assert_napi_ok!(napi_create_external_arraybuffer(
    env,
    buf.as_ptr() as _,
    3,
    Some(finalize_cb_vec),
    ptr::null_mut(),
    &mut result
  ));
  std::mem::forget(buf);

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_bind_finalizer", test_bind_finalizer),
    napi_new_property!(env, "test_external_finalizer", test_external_finalizer),
    napi_new_property!(env, "test_external_buffer", test_external_buffer),
    napi_new_property!(
      env,
      "test_external_arraybuffer",
      test_external_arraybuffer
    ),
    napi_new_property!(
      env,
      "test_static_external_buffer",
      test_static_external_buffer
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
