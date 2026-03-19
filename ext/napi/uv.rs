// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;

use deno_core::parking_lot::Mutex;
use deno_core::uv_compat;

use crate::util::SendPtr;
use crate::*;

fn assert_ok(res: c_int) -> c_int {
  if res != 0 {
    log::error!("bad result in uv polyfill: {res}");
    // don't panic because that might unwind into
    // c/c++
    std::process::abort();
  }
  res
}

use std::ffi::c_int;

use js_native_api::napi_create_string_utf8;
use node_api::napi_create_async_work;
use node_api::napi_delete_async_work;

// ---------------------------------------------------------------------------
// Mutex
// ---------------------------------------------------------------------------

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

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_init(lock: *mut uv_mutex_t) -> c_int {
  unsafe {
    addr_of_mut!((*lock).mutex).write(Mutex::new(()));
    0
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_lock(lock: *mut uv_mutex_t) {
  unsafe {
    let guard = (*lock).mutex.lock();
    // forget the guard so it doesn't unlock when it goes out of scope.
    // we're going to unlock it manually
    std::mem::forget(guard);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_unlock(lock: *mut uv_mutex_t) {
  unsafe {
    (*lock).mutex.force_unlock();
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_destroy(_lock: *mut uv_mutex_t) {
  // no cleanup required
}

// ---------------------------------------------------------------------------
// Handle types & sizes (must match real libuv ABI)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "variants represent libuv enum values")]
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
  // public members (libuv ABI: data at offset 0, loop at offset 8, type at offset 16)
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

// ---------------------------------------------------------------------------
// Async handle
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Timer handle
// ---------------------------------------------------------------------------

/// NAPI timer handle. Libuv-ABI-compatible public fields at the correct
/// offsets, with a pointer to a shadow core `uv_compat::uv_timer_t` stored
/// in the private area. Must be smaller than libuv's `uv_timer_t` since
/// addons allocate libuv-sized memory.
#[repr(C)]
struct uv_timer_t {
  // public members (libuv ABI)
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,
  // private: pointer to core-layout shadow handle
  core_handle: *mut uv_compat::uv_timer_t,
  // original callback from the addon
  timer_cb: Option<unsafe extern "C" fn(*mut uv_timer_t)>,
}

// ---------------------------------------------------------------------------
// Idle handle
// ---------------------------------------------------------------------------

#[repr(C)]
struct uv_idle_t {
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,
  core_handle: *mut uv_compat::uv_idle_t,
  idle_cb: Option<unsafe extern "C" fn(*mut uv_idle_t)>,
}

// ---------------------------------------------------------------------------
// Check handle
// ---------------------------------------------------------------------------

#[repr(C)]
struct uv_check_t {
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,
  core_handle: *mut uv_compat::uv_check_t,
  check_cb: Option<unsafe extern "C" fn(*mut uv_check_t)>,
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

type uv_loop_t = Env;
type uv_async_cb = extern "C" fn(handle: *mut uv_async_t);
type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);

/// Get the core `uv_loop_t` pointer from a NAPI `uv_loop_t` (which is `Env`).
///
/// # Safety
/// `napi_loop` must be a valid pointer to an `Env`.
#[inline]
unsafe fn get_core_loop(
  napi_loop: *mut uv_loop_t,
) -> *mut uv_compat::uv_loop_t {
  unsafe { (*napi_loop).uv_loop_ptr }
}

// ---------------------------------------------------------------------------
// Async implementation
// ---------------------------------------------------------------------------

#[unsafe(export_name = "uv_async_init")]
unsafe extern "C" fn _napi_uv_async_init(
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

    // In libuv, uv_async_init starts the handle and keeps the event loop
    // alive until uv_close is called. Ref the event loop to match this.
    let env = &mut *r#loop;
    env.external_ops_tracker.ref_op();

    -res
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_async_send(handle: *mut uv_async_t) -> c_int {
  // Dispatch directly to the main thread. Unlike napi_queue_async_work (which
  // runs `execute` on a worker thread), uv_async callbacks need V8 access so
  // they must run on the main thread.
  unsafe {
    let env = &mut *(*handle).r#loop;
    let handle = SendPtr(handle as *const uv_async_t);
    env.async_work_sender.spawn(move |_| {
      let handle = handle.take() as *mut uv_async_t;
      ((*handle).async_cb)(handle);
    });
  }
  0
}

unsafe extern "C" fn async_exec_wrap(_env: napi_env, data: *mut c_void) {
  let data: *mut uv_async_t = data.cast();
  unsafe {
    ((*data).async_cb)(data);
  }
}

// ---------------------------------------------------------------------------
// Timer implementation (delegates to core uv_compat via shadow handles)
// ---------------------------------------------------------------------------

/// Callback trampoline: core fires this on the shadow handle, we translate
/// back to the NAPI handle and invoke the addon's original callback.
unsafe extern "C" fn timer_cb_trampoline(
  core_handle: *mut uv_compat::uv_timer_t,
) {
  unsafe {
    let napi_handle = (*core_handle).data as *mut uv_timer_t;
    if let Some(cb) = (*napi_handle).timer_cb {
      cb(napi_handle);
    }
  }
}

#[unsafe(export_name = "uv_timer_init")]
unsafe extern "C" fn _napi_uv_timer_init(
  r#loop: *mut uv_loop_t,
  handle: *mut uv_timer_t,
) -> c_int {
  unsafe {
    let core_loop = get_core_loop(r#loop);
    if core_loop.is_null() {
      return -1;
    }

    // Allocate a core-layout shadow timer handle.
    let core_handle =
      Box::into_raw(Box::new(std::mem::zeroed::<uv_compat::uv_timer_t>()));
    let rc = uv_compat::uv_timer_init(core_loop, core_handle);
    if rc != 0 {
      drop(Box::from_raw(core_handle));
      return rc;
    }
    // Store NAPI handle pointer in core handle's data field for the trampoline.
    (*core_handle).data = handle as *mut c_void;

    // Initialize the NAPI handle.
    addr_of_mut!((*handle).r#loop).write(r#loop);
    addr_of_mut!((*handle).r#type).write(uv_handle_type::UV_TIMER);
    addr_of_mut!((*handle).core_handle).write(core_handle);
    addr_of_mut!((*handle).timer_cb).write(None);
    0
  }
}

#[unsafe(export_name = "uv_timer_start")]
unsafe extern "C" fn _napi_uv_timer_start(
  handle: *mut uv_timer_t,
  cb: unsafe extern "C" fn(*mut uv_timer_t),
  timeout: u64,
  repeat: u64,
) -> c_int {
  unsafe {
    (*handle).timer_cb = Some(cb);
    uv_compat::uv_timer_start(
      (*handle).core_handle,
      timer_cb_trampoline,
      timeout,
      repeat,
    )
  }
}

#[unsafe(export_name = "uv_timer_stop")]
unsafe extern "C" fn _napi_uv_timer_stop(handle: *mut uv_timer_t) -> c_int {
  unsafe { uv_compat::uv_timer_stop((*handle).core_handle) }
}

#[unsafe(export_name = "uv_timer_again")]
unsafe extern "C" fn _napi_uv_timer_again(handle: *mut uv_timer_t) -> c_int {
  unsafe { uv_compat::uv_timer_again((*handle).core_handle) }
}

#[unsafe(export_name = "uv_timer_set_repeat")]
unsafe extern "C" fn _napi_uv_timer_set_repeat(
  handle: *mut uv_timer_t,
  repeat: u64,
) {
  unsafe { uv_compat::uv_timer_set_repeat((*handle).core_handle, repeat) }
}

#[unsafe(export_name = "uv_timer_get_repeat")]
unsafe extern "C" fn _napi_uv_timer_get_repeat(
  handle: *const uv_timer_t,
) -> u64 {
  unsafe { uv_compat::uv_timer_get_repeat((*handle).core_handle) }
}

// ---------------------------------------------------------------------------
// Idle implementation
// ---------------------------------------------------------------------------

unsafe extern "C" fn idle_cb_trampoline(
  core_handle: *mut uv_compat::uv_idle_t,
) {
  unsafe {
    let napi_handle = (*core_handle).data as *mut uv_idle_t;
    if let Some(cb) = (*napi_handle).idle_cb {
      cb(napi_handle);
    }
  }
}

#[unsafe(export_name = "uv_idle_init")]
unsafe extern "C" fn _napi_uv_idle_init(
  r#loop: *mut uv_loop_t,
  handle: *mut uv_idle_t,
) -> c_int {
  unsafe {
    let core_loop = get_core_loop(r#loop);
    if core_loop.is_null() {
      return -1;
    }

    let core_handle =
      Box::into_raw(Box::new(std::mem::zeroed::<uv_compat::uv_idle_t>()));
    let rc = uv_compat::uv_idle_init(core_loop, core_handle);
    if rc != 0 {
      drop(Box::from_raw(core_handle));
      return rc;
    }
    (*core_handle).data = handle as *mut c_void;

    addr_of_mut!((*handle).r#loop).write(r#loop);
    addr_of_mut!((*handle).r#type).write(uv_handle_type::UV_IDLE);
    addr_of_mut!((*handle).core_handle).write(core_handle);
    addr_of_mut!((*handle).idle_cb).write(None);
    0
  }
}

#[unsafe(export_name = "uv_idle_start")]
unsafe extern "C" fn _napi_uv_idle_start(
  handle: *mut uv_idle_t,
  cb: unsafe extern "C" fn(*mut uv_idle_t),
) -> c_int {
  unsafe {
    (*handle).idle_cb = Some(cb);
    uv_compat::uv_idle_start((*handle).core_handle, idle_cb_trampoline)
  }
}

#[unsafe(export_name = "uv_idle_stop")]
unsafe extern "C" fn _napi_uv_idle_stop(handle: *mut uv_idle_t) -> c_int {
  unsafe { uv_compat::uv_idle_stop((*handle).core_handle) }
}

// ---------------------------------------------------------------------------
// Check implementation
// ---------------------------------------------------------------------------

unsafe extern "C" fn check_cb_trampoline(
  core_handle: *mut uv_compat::uv_check_t,
) {
  unsafe {
    let napi_handle = (*core_handle).data as *mut uv_check_t;
    if let Some(cb) = (*napi_handle).check_cb {
      cb(napi_handle);
    }
  }
}

#[unsafe(export_name = "uv_check_init")]
unsafe extern "C" fn _napi_uv_check_init(
  r#loop: *mut uv_loop_t,
  handle: *mut uv_check_t,
) -> c_int {
  unsafe {
    let core_loop = get_core_loop(r#loop);
    if core_loop.is_null() {
      return -1;
    }

    let core_handle =
      Box::into_raw(Box::new(std::mem::zeroed::<uv_compat::uv_check_t>()));
    let rc = uv_compat::uv_check_init(core_loop, core_handle);
    if rc != 0 {
      drop(Box::from_raw(core_handle));
      return rc;
    }
    (*core_handle).data = handle as *mut c_void;

    addr_of_mut!((*handle).r#loop).write(r#loop);
    addr_of_mut!((*handle).r#type).write(uv_handle_type::UV_CHECK);
    addr_of_mut!((*handle).core_handle).write(core_handle);
    addr_of_mut!((*handle).check_cb).write(None);
    0
  }
}

#[unsafe(export_name = "uv_check_start")]
unsafe extern "C" fn _napi_uv_check_start(
  handle: *mut uv_check_t,
  cb: unsafe extern "C" fn(*mut uv_check_t),
) -> c_int {
  unsafe {
    (*handle).check_cb = Some(cb);
    uv_compat::uv_check_start((*handle).core_handle, check_cb_trampoline)
  }
}

#[unsafe(export_name = "uv_check_stop")]
unsafe extern "C" fn _napi_uv_check_stop(handle: *mut uv_check_t) -> c_int {
  unsafe { uv_compat::uv_check_stop((*handle).core_handle) }
}

// ---------------------------------------------------------------------------
// uv_close — handles all NAPI handle types
// ---------------------------------------------------------------------------

/// Per-handle close state: we need to first close the core shadow handle,
/// then invoke the addon's close callback with the NAPI handle.
struct CloseCtx {
  napi_handle: *mut uv_handle_t,
  close_cb: Option<uv_close_cb>,
}

unsafe extern "C" fn shadow_close_cb(core_handle: *mut uv_compat::uv_handle_t) {
  unsafe {
    let ctx = Box::from_raw((*core_handle).data as *mut CloseCtx);
    // Free the core shadow handle.
    drop(Box::from_raw(core_handle));
    // Invoke the addon's close callback with the NAPI handle.
    if let Some(cb) = ctx.close_cb {
      cb(ctx.napi_handle);
    }
  }
}

#[unsafe(export_name = "uv_close")]
unsafe extern "C" fn _napi_uv_close(
  handle: *mut uv_handle_t,
  close: Option<uv_close_cb>,
) {
  unsafe {
    if handle.is_null() {
      if let Some(close) = close {
        close(handle);
      }
      return;
    }
    match (*handle).r#type {
      uv_handle_type::UV_ASYNC => {
        let handle: *mut uv_async_t = handle.cast();
        napi_delete_async_work((*handle).r#loop, (*handle).work);
        let env = &mut *(*handle).r#loop;
        env.external_ops_tracker.unref_op();
        if let Some(close) = close {
          close(handle as *mut uv_handle_t);
        }
      }
      uv_handle_type::UV_TIMER => {
        let timer = handle as *mut uv_timer_t;
        let core_handle = (*timer).core_handle;
        let ctx = Box::into_raw(Box::new(CloseCtx {
          napi_handle: handle,
          close_cb: close,
        }));
        (*core_handle).data = ctx as *mut c_void;
        uv_compat::uv_close(
          core_handle as *mut uv_compat::uv_handle_t,
          Some(shadow_close_cb),
        );
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *mut uv_idle_t;
        let core_handle = (*idle).core_handle;
        let ctx = Box::into_raw(Box::new(CloseCtx {
          napi_handle: handle,
          close_cb: close,
        }));
        (*core_handle).data = ctx as *mut c_void;
        uv_compat::uv_close(
          core_handle as *mut uv_compat::uv_handle_t,
          Some(shadow_close_cb),
        );
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *mut uv_check_t;
        let core_handle = (*check).core_handle;
        let ctx = Box::into_raw(Box::new(CloseCtx {
          napi_handle: handle,
          close_cb: close,
        }));
        (*core_handle).data = ctx as *mut c_void;
        uv_compat::uv_close(
          core_handle as *mut uv_compat::uv_handle_t,
          Some(shadow_close_cb),
        );
      }
      _ => {
        // Unknown handle type — just invoke the close callback.
        if let Some(close) = close {
          close(handle);
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// uv_ref / uv_unref — forward to core shadow handle when available
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_ref(handle: *mut uv_handle_t) {
  unsafe {
    if handle.is_null() {
      return;
    }
    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        let timer = handle as *mut uv_timer_t;
        uv_compat::uv_ref((*timer).core_handle as *mut uv_compat::uv_handle_t);
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *mut uv_idle_t;
        uv_compat::uv_ref((*idle).core_handle as *mut uv_compat::uv_handle_t);
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *mut uv_check_t;
        uv_compat::uv_ref((*check).core_handle as *mut uv_compat::uv_handle_t);
      }
      // For async and other handle types without core shadow handles, no-op.
      _ => {}
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_unref(handle: *mut uv_handle_t) {
  unsafe {
    if handle.is_null() {
      return;
    }
    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        let timer = handle as *mut uv_timer_t;
        uv_compat::uv_unref(
          (*timer).core_handle as *mut uv_compat::uv_handle_t,
        );
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *mut uv_idle_t;
        uv_compat::uv_unref((*idle).core_handle as *mut uv_compat::uv_handle_t);
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *mut uv_check_t;
        uv_compat::uv_unref(
          (*check).core_handle as *mut uv_compat::uv_handle_t,
        );
      }
      _ => {}
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_has_ref(handle: *const uv_handle_t) -> c_int {
  unsafe {
    if handle.is_null() {
      return 0;
    }
    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        let timer = handle as *const uv_timer_t;
        uv_compat::uv_has_ref(
          (*timer).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *const uv_idle_t;
        uv_compat::uv_has_ref(
          (*idle).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *const uv_check_t;
        uv_compat::uv_has_ref(
          (*check).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      // Async handles are always ref'd (managed by external_ops_tracker).
      _ => 1,
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_active(handle: *const uv_handle_t) -> c_int {
  unsafe {
    if handle.is_null() {
      return 0;
    }
    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        let timer = handle as *const uv_timer_t;
        uv_compat::uv_is_active(
          (*timer).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *const uv_idle_t;
        uv_compat::uv_is_active(
          (*idle).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *const uv_check_t;
        uv_compat::uv_is_active(
          (*check).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      _ => 0,
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_closing(handle: *const uv_handle_t) -> c_int {
  unsafe {
    if handle.is_null() {
      return 0;
    }
    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        let timer = handle as *const uv_timer_t;
        uv_compat::uv_is_closing(
          (*timer).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_IDLE => {
        let idle = handle as *const uv_idle_t;
        uv_compat::uv_is_closing(
          (*idle).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      uv_handle_type::UV_CHECK => {
        let check = handle as *const uv_check_t;
        uv_compat::uv_is_closing(
          (*check).core_handle as *const uv_compat::uv_handle_t,
        )
      }
      _ => 0,
    }
  }
}

// ---------------------------------------------------------------------------
// Misc utilities
// ---------------------------------------------------------------------------

#[cfg(unix)]
type uv_pid_t = c_int;
#[cfg(windows)]
type uv_pid_t = c_int;

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_os_getpid() -> uv_pid_t {
  std::process::id() as uv_pid_t
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    // NAPI handle sizes must be <= libuv sizes since addons allocate
    // libuv-sized memory and we only use the first N bytes.
    assert!(
      std::mem::size_of::<uv_timer_t>()
        <= std::mem::size_of::<libuv_sys_lite::uv_timer_t>()
    );
    assert!(
      std::mem::size_of::<uv_idle_t>()
        <= std::mem::size_of::<libuv_sys_lite::uv_idle_t>()
    );
    assert!(
      std::mem::size_of::<uv_check_t>()
        <= std::mem::size_of::<libuv_sys_lite::uv_check_t>()
    );
  }
}
