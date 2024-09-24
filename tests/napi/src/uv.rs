// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use libuv_sys_lite::uv_async_init;
use libuv_sys_lite::uv_async_t;
use napi_sys::ValueType::napi_function;
use napi_sys::*;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::ptr;
use std::ptr::addr_of_mut;

extern "C" {}

pub struct Baton {
  called: bool,
  task: *mut uv_async_t,
}

fn new_raw<T>(t: T) -> *mut T {
  Box::into_raw(Box::new(t))
}

unsafe extern "C" fn callback(handle: *mut uv_async_t) {}

extern "C" fn test_async_work(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = MaybeUninit::<*mut napi_sys::uv_loop_s>::uninit();
  assert_napi_ok!(napi_get_uv_event_loop(env, loop_.as_mut_ptr()));
  let data = new_raw(4u64);
  unsafe {
    let mut loop_ = loop_.assume_init();
    let mut uv_async = new_raw(MaybeUninit::<uv_async_t>::uninit());
    let mut casted = uv_async.cast::<uv_async_t>();
    addr_of_mut!((*casted).data).write(data);
    assert_eq!(
      uv_async_init(loop_.cast(), (*uv_async).as_mut_ptr(), Some(callback)),
      0
    );
    let mut uv_async = uv_async.cast::<uv_async_t>();
    uv_async
  }

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
