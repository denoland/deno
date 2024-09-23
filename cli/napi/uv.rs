// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

#[cfg(unix)]
mod mutex {
  use super::*;
  type uv_mutex_t = libc::pthread_mutex_t;
  #[no_mangle]
  unsafe extern "C" fn uv_mutex_init(lock: *mut uv_mutex_t) -> c_int {
    use std::mem::MaybeUninit;
    let mut attr = MaybeUninit::<libc::pthread_mutexattr_t>::uninit();
    unsafe {
      assert_ok(libc::pthread_mutexattr_init(attr.as_mut_ptr()));
      let mut attr = attr.assume_init();
      let attr_ptr = addr_of_mut!(attr);
      assert_ok(libc::pthread_mutexattr_settype(
        attr_ptr,
        libc::PTHREAD_MUTEX_ERRORCHECK,
      ));
      let err = libc::pthread_mutex_init(lock, attr_ptr);
      assert_ok(libc::pthread_mutexattr_destroy(attr_ptr));
      if libc::EDOM > 0 {
        err
      } else {
        -err
      }
    }
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_lock(lock: *mut uv_mutex_t) {
    unsafe {
      assert_ok(libc::pthread_mutex_lock(lock));
    }
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_unlock(lock: *mut uv_mutex_t) {
    unsafe {
      assert_ok(libc::pthread_mutex_unlock(lock));
    }
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_destroy(lock: *mut uv_mutex_t) {
    unsafe {
      assert_ok(libc::pthread_mutex_destroy(lock));
    }
  }
}
#[cfg(windows)]
mod mutex {
  use super::*;
  use windows_sys::Win32::System::Threading as win;
  type uv_mutex_t = win::CRITICAL_SECTION;

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_init(lock: *mut uv_mutex_t) -> c_int {
    unsafe {
      win::InitializeCriticalSection(lock);
    }
    0
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_lock(lock: *mut uv_mutex_t) {
    unsafe {
      win::EnterCriticalSection(lock);
    }
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_unlock(lock: *mut uv_mutex_t) {
    unsafe {
      win::LeaveCriticalSection(lock);
    }
  }

  #[no_mangle]
  unsafe extern "C" fn uv_mutex_destroy(lock: *mut uv_mutex_t) {
    unsafe {
      win::DeleteCriticalSection(lock);
    }
  }
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

  _padding: [MaybeUninit<u8>; const {
    UV_HANDLE_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>()
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
  _padding: [MaybeUninit<u8>; const {
    UV_ASYNC_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>()
      - size_of::<uv_async_cb>()
      - size_of::<napi_async_work>()
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
