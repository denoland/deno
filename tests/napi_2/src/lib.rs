// Copyright 2018-2026 the Deno authors. MIT license.

//! A second NAPI test addon for cross-env wrap/unwrap testing.
//! Objects wrapped by test_napi should be unwrappable by this addon
//! and vice versa, matching Node.js behavior where the napi_wrap
//! Private key is per-isolate (not per-addon).

#![allow(clippy::all, reason = "test napi code")]
#![allow(clippy::undocumented_unsafe_blocks, reason = "test napi code")]
#![allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]

use std::ffi::c_void;
use std::ptr;

use napi_sys::*;

macro_rules! assert_napi_ok {
  ($call: expr) => {{
    assert_eq!(
      {
        #[allow(
          unused_unsafe,
          reason = "napi_sys safe fn in unsafe extern blocks"
        )]
        unsafe {
          $call
        }
      },
      napi_sys::Status::napi_ok
    );
  }};
}

macro_rules! napi_get_callback_info {
  ($env: expr, $callback_info: expr, $size: literal) => {{
    let mut args = [std::ptr::null_mut(); $size];
    let mut argc = $size;
    let mut this = std::ptr::null_mut();
    assert_napi_ok!(napi_get_cb_info(
      $env,
      $callback_info,
      &mut argc,
      args.as_mut_ptr(),
      &mut this,
      std::ptr::null_mut(),
    ));
    (args, argc, this)
  }};
}

/// Wrap a JS object with an i32 value using napi_wrap.
extern "C" fn wrap_object(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut value: i32 = 0;
  assert_napi_ok!(napi_get_value_int32(env, args[1], &mut value));

  let data = Box::into_raw(Box::new(value)) as *mut c_void;
  assert_napi_ok!(napi_wrap(
    env,
    args[0],
    data,
    None,
    ptr::null_mut(),
    ptr::null_mut()
  ));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Unwrap a JS object and return the i32 value.
/// Returns null if unwrap fails (e.g. object was not wrapped).
extern "C" fn unwrap_object(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut data: *mut c_void = ptr::null_mut();
  let status = unsafe { napi_unwrap(env, args[0], &mut data) };
  if status != napi_sys::Status::napi_ok {
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_null(env, &mut result));
    return result;
  }

  let value = unsafe { *(data as *const i32) };
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, value, &mut result));
  result
}

#[unsafe(no_mangle)]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  _: napi_value,
) -> napi_value {
  let mut exports = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut exports));

  let properties = &[
    napi_property_descriptor {
      utf8name: c"wrapObject".as_ptr(),
      name: ptr::null_mut(),
      method: Some(wrap_object),
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: 0,
      value: ptr::null_mut(),
    },
    napi_property_descriptor {
      utf8name: c"unwrapObject".as_ptr(),
      name: ptr::null_mut(),
      method: Some(unwrap_object),
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: 0,
      value: ptr::null_mut(),
    },
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));

  exports
}
