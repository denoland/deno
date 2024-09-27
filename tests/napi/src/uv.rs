// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use libuv_sys_lite::uv_async_init;
use libuv_sys_lite::uv_async_t;
use libuv_sys_lite::uv_close;
use libuv_sys_lite::uv_handle_t;
use libuv_sys_lite::uv_mutex_destroy;
use libuv_sys_lite::uv_mutex_lock;
use libuv_sys_lite::uv_mutex_t;
use libuv_sys_lite::uv_mutex_unlock;
use napi_sys::*;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::addr_of_mut;
use std::ptr::null_mut;
use std::time::Duration;

struct KeepAlive {
  tsfn: napi_threadsafe_function,
}

impl KeepAlive {
  fn new(env: napi_env) -> Self {
    let mut name = null_mut();
    assert_napi_ok!(napi_create_string_utf8(
      env,
      c"test_uv_async".as_ptr(),
      13,
      &mut name
    ));

    unsafe extern "C" fn dummy(
      _env: napi_env,
      _cb: napi_callback_info,
    ) -> napi_value {
      ptr::null_mut()
    }

    let mut func = null_mut();
    assert_napi_ok!(napi_create_function(
      env,
      c"dummy".as_ptr(),
      usize::MAX,
      Some(dummy),
      null_mut(),
      &mut func,
    ));

    let mut tsfn = null_mut();
    assert_napi_ok!(napi_create_threadsafe_function(
      env,
      func,
      null_mut(),
      name,
      0,
      1,
      null_mut(),
      None,
      null_mut(),
      None,
      &mut tsfn,
    ));
    assert_napi_ok!(napi_ref_threadsafe_function(env, tsfn));
    Self { tsfn }
  }
}

impl Drop for KeepAlive {
  fn drop(&mut self) {
    assert_napi_ok!(napi_release_threadsafe_function(
      self.tsfn,
      ThreadsafeFunctionReleaseMode::release,
    ));
  }
}

struct Async {
  mutex: *mut uv_mutex_t,
  env: napi_env,
  value: u32,
  callback: napi_ref,
  _keep_alive: KeepAlive,
}

#[derive(Clone, Copy)]
struct UvAsyncPtr(*mut uv_async_t);

unsafe impl Send for UvAsyncPtr {}

fn new_raw<T>(t: T) -> *mut T {
  Box::into_raw(Box::new(t))
}

unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
  let handle = handle.cast::<uv_async_t>();
  let async_ = (*handle).data as *mut Async;
  let env = (*async_).env;
  assert_napi_ok!(napi_delete_reference(env, (*async_).callback));

  uv_mutex_destroy((*async_).mutex);
  let _ = Box::from_raw((*async_).mutex);
  let _ = Box::from_raw(async_);
  let _ = Box::from_raw(handle);
}

unsafe extern "C" fn callback(handle: *mut uv_async_t) {
  eprintln!("callback");
  let async_ = (*handle).data as *mut Async;
  uv_mutex_lock((*async_).mutex);
  let env = (*async_).env;
  let mut js_cb = null_mut();
  assert_napi_ok!(napi_get_reference_value(
    env,
    (*async_).callback,
    &mut js_cb
  ));
  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  let value = (*async_).value;
  eprintln!("value is {value}");
  let mut value_js = ptr::null_mut();
  assert_napi_ok!(napi_create_uint32(env, value, &mut value_js));
  let args = &[value_js];
  assert_napi_ok!(napi_call_function(
    env,
    global,
    js_cb,
    1,
    args.as_ptr(),
    &mut result,
  ));
  uv_mutex_unlock((*async_).mutex);
  if value == 5 {
    uv_close(handle.cast(), Some(close_cb));
  }
}

unsafe fn uv_async_send(ptr: UvAsyncPtr) {
  assert_napi_ok!(libuv_sys_lite::uv_async_send(ptr.0));
}

fn make_uv_mutex() -> *mut uv_mutex_t {
  let mutex = new_raw(MaybeUninit::<uv_mutex_t>::uninit());
  assert_napi_ok!(libuv_sys_lite::uv_mutex_init(mutex.cast()));
  mutex.cast()
}

#[allow(unused_unsafe)]
extern "C" fn test_uv_async(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
  let uv_async = new_raw(MaybeUninit::<uv_async_t>::uninit());
  let uv_async = uv_async.cast::<uv_async_t>();
  let mut js_cb = null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  // let mut tsfn = null_mut();

  let data = new_raw(Async {
    env,
    callback: js_cb,
    mutex: make_uv_mutex(),
    value: 0,
    _keep_alive: KeepAlive::new(env),
  });
  unsafe {
    addr_of_mut!((*uv_async).data).write(data.cast());
    assert_napi_ok!(uv_async_init(loop_.cast(), uv_async, Some(callback)));
    let uv_async = UvAsyncPtr(uv_async);
    std::thread::spawn({
      move || {
        let data = (*uv_async.0).data as *mut Async;
        for _ in 0..5 {
          uv_mutex_lock((*data).mutex);
          (*data).value += 1;
          uv_mutex_unlock((*data).mutex);
          std::thread::sleep(Duration::from_millis(10));
          uv_async_send(uv_async);
        }
      }
    });
  }

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(env, "test_uv_async", test_uv_async)];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
