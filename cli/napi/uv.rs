// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::parking_lot::Mutex;
use deno_runtime::deno_napi::*;
use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;

#[allow(clippy::print_stderr)]
fn assert_ok(res: c_int) -> c_int {
  if res != 0 {
    eprintln!("bad result in uv polyfill: {res}");
    // don't panic because that might unwind into
    // c/c++
    std::process::abort();
  }
  res
}

use crate::napi::js_native_api::napi_create_string_utf8;
use crate::napi::node_api::napi_create_async_work;
use crate::napi::node_api::napi_delete_async_work;
use crate::napi::node_api::napi_queue_async_work;
use std::ffi::c_int;

const UV_MUTEX_SIZE: usize = {
  #[cfg(unix)]
  {
    std::mem::size_of::<libc::pthread_mutex_t>()
  }
  #[cfg(windows)]
  {
    std::mem::size_of::<windows_sys::Win32::System::Threading::CRITICAL_SECTION>(
    )
  }
};

#[repr(C)]
struct uv_mutex_t {
  mutex: Mutex<()>,
  _padding: [MaybeUninit<usize>; const {
    (UV_MUTEX_SIZE - size_of::<Mutex<()>>()) / size_of::<usize>()
  }],
}

#[no_mangle]
unsafe extern "C" fn uv_mutex_init(lock: *mut uv_mutex_t) -> c_int {
  unsafe {
    addr_of_mut!((*lock).mutex).write(Mutex::new(()));
    0
  }
}

#[no_mangle]
unsafe extern "C" fn uv_mutex_lock(lock: *mut uv_mutex_t) {
  unsafe {
    let guard = (*lock).mutex.lock();
    // forget the guard so it doesn't unlock when it goes out of scope.
    // we're going to unlock it manually
    std::mem::forget(guard);
  }
}

#[no_mangle]
unsafe extern "C" fn uv_mutex_unlock(lock: *mut uv_mutex_t) {
  unsafe {
    (*lock).mutex.force_unlock();
  }
}

#[no_mangle]
unsafe extern "C" fn uv_mutex_destroy(_lock: *mut uv_mutex_t) {
  // no cleanup required
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum uv_handle_type {
  UV_UNKNOWN_HANDLE = 0,
  UV_ASYNC,
  UV_CHECK,
  UV_FS_EVENT,
  UV_FS_POLL,
  UV_HANDLE,
  UV_IDLE,
  UV_NAMED_PIPE,
  UV_POLL,
  UV_PREPARE,
  UV_PROCESS,
  UV_STREAM,
  UV_TCP,
  UV_TIMER,
  UV_TTY,
  UV_UDP,
  UV_SIGNAL,
  UV_FILE,
  UV_HANDLE_TYPE_MAX,
}

const UV_HANDLE_SIZE: usize = 96;

#[repr(C)]
struct uv_handle_t {
  // public members
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,

  _padding: [MaybeUninit<usize>; const {
    (UV_HANDLE_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>())
      / size_of::<usize>()
  }],
}

#[cfg(unix)]
const UV_ASYNC_SIZE: usize = 128;

#[cfg(windows)]
const UV_ASYNC_SIZE: usize = 224;

#[repr(C)]
struct uv_async_t {
  // public members
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,
  // private
  async_cb: uv_async_cb,
  work: napi_async_work,
  _padding: [MaybeUninit<usize>; const {
    (UV_ASYNC_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>()
      - size_of::<uv_async_cb>()
      - size_of::<napi_async_work>())
      / size_of::<usize>()
  }],
}

type uv_loop_t = Env;
type uv_async_cb = extern "C" fn(handle: *mut uv_async_t);
#[no_mangle]
unsafe extern "C" fn uv_async_init(
  r#loop: *mut uv_loop_t,
  // probably uninitialized
  r#async: *mut uv_async_t,
  async_cb: uv_async_cb,
) -> c_int {
  unsafe {
    addr_of_mut!((*r#async).r#loop).write(r#loop);
    addr_of_mut!((*r#async).r#type).write(uv_handle_type::UV_ASYNC);
    addr_of_mut!((*r#async).async_cb).write(async_cb);

    let mut resource_name: MaybeUninit<napi_value> = MaybeUninit::uninit();
    assert_ok(napi_create_string_utf8(
      r#loop,
      c"uv_async".as_ptr(),
      usize::MAX,
      resource_name.as_mut_ptr(),
    ));
    let resource_name = resource_name.assume_init();

    let res = napi_create_async_work(
      r#loop,
      None::<v8::Local<'static, v8::Value>>.into(),
      resource_name,
      Some(async_exec_wrap),
      None,
      r#async.cast(),
      addr_of_mut!((*r#async).work),
    );
    -res
  }
}

#[no_mangle]
unsafe extern "C" fn uv_async_send(handle: *mut uv_async_t) -> c_int {
  unsafe { -napi_queue_async_work((*handle).r#loop, (*handle).work) }
}

type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);

#[no_mangle]
unsafe extern "C" fn uv_close(handle: *mut uv_handle_t, close: uv_close_cb) {
  unsafe {
    if handle.is_null() {
      close(handle);
      return;
    }
    if let uv_handle_type::UV_ASYNC = (*handle).r#type {
      let handle: *mut uv_async_t = handle.cast();
      napi_delete_async_work((*handle).r#loop, (*handle).work);
    }
    close(handle);
  }
}

unsafe extern "C" fn async_exec_wrap(_env: napi_env, data: *mut c_void) {
  let data: *mut uv_async_t = data.cast();
  unsafe {
    ((*data).async_cb)(data);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sizes() {
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_mutex_t>(),
      UV_MUTEX_SIZE
    );
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_handle_t>(),
      UV_HANDLE_SIZE
    );
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_async_t>(),
      UV_ASYNC_SIZE
    );
    assert_eq!(std::mem::size_of::<uv_mutex_t>(), UV_MUTEX_SIZE);
    assert_eq!(std::mem::size_of::<uv_handle_t>(), UV_HANDLE_SIZE);
    assert_eq!(std::mem::size_of::<uv_async_t>(), UV_ASYNC_SIZE);
  }
}
