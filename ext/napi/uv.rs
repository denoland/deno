// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;
use std::sync::OnceLock;
use std::time::Instant;

use deno_core::parking_lot::Mutex;

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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
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

type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);

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
        // Unref the event loop to match the ref in uv_async_init.
        let env = &mut *(*handle).r#loop;
        env.external_ops_tracker.unref_op();
      }
      uv_handle_type::UV_TIMER => {
        let handle: *mut uv_timer_t = handle.cast();
        timer_drop(handle);
      }
      _ => {}
    }
    if let Some(close) = close {
      close(handle);
    }
  }
}

// ---------- uv timer / cpu_info / misc polyfills ----------
//
// The Sentry profiling-node native addon (and a handful of other native
// addons that link against libuv directly) reaches into libuv for handle
// types beyond `uv_async_t` and `uv_mutex_t`. Deno does not run on libuv,
// so we satisfy these symbols with lightweight polyfills:
//
// * `uv_hrtime` returns a monotonic timestamp.
// * `uv_handle_set_data`, `uv_ref`, `uv_unref`, `uv_is_closing` mirror the
//   trivial libuv behavior.
// * `uv_cpu_info` returns an error so callers degrade gracefully (the
//   profiler skips per-tick CPU stats but still produces a valid profile).
// * `uv_timer_*` is a no-op stub. The Sentry profiler uses the timer to
//   collect periodic heap/CPU measurements; the CPU profile itself is
//   captured via `v8::CpuProfiler::StartProfiling` and does not depend on
//   the timer firing. With a no-op timer the resulting profile is valid
//   but lacks measurement samples.

#[cfg(unix)]
const UV_TIMER_SIZE: usize = 152;

#[cfg(windows)]
const UV_TIMER_SIZE: usize = 160;

type uv_timer_cb = Option<unsafe extern "C" fn(handle: *mut uv_timer_t)>;

#[repr(C)]
struct uv_timer_t {
  // public members (must match libuv layout)
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,

  _padding: [MaybeUninit<usize>; const {
    (UV_TIMER_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>())
      / size_of::<usize>()
  }],
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_init(
  r#loop: *mut uv_loop_t,
  timer: *mut uv_timer_t,
) -> c_int {
  // Zero the public fields. Internal libuv layout is opaque to the addon,
  // and our polyfill does not track per-timer state (see notes above), so
  // there is nothing else to initialize.
  unsafe {
    addr_of_mut!((*timer).data).write(std::ptr::null_mut());
    addr_of_mut!((*timer).r#loop).write(r#loop);
    addr_of_mut!((*timer).r#type).write(uv_handle_type::UV_TIMER);
  }
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_start(
  _handle: *mut uv_timer_t,
  _cb: uv_timer_cb,
  _timeout_ms: u64,
  _repeat_ms: u64,
) -> c_int {
  // No-op. See the module comment above.
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_stop(_handle: *mut uv_timer_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_set_repeat(
  _handle: *mut uv_timer_t,
  _repeat_ms: u64,
) {
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_get_repeat(_handle: *const uv_timer_t) -> u64 {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_again(_handle: *mut uv_timer_t) -> c_int {
  0
}

fn timer_drop(_handle: *mut uv_timer_t) {
  // Currently no per-timer state to release. Reserved for a future real
  // timer polyfill so uv_close can clean up.
}

// uv_hrtime returns nanoseconds since an arbitrary monotonic origin. We
// peg the origin to the first call.
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_hrtime() -> u64 {
  static START: OnceLock<Instant> = OnceLock::new();
  let start = START.get_or_init(Instant::now);
  start.elapsed().as_nanos() as u64
}

// Many native addons reach for `uv_default_loop()` because they predate
// `napi_get_uv_event_loop`. We return a sentinel that is sufficient for
// the no-op timer/handle polyfills below.
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_default_loop() -> *mut uv_loop_t {
  std::ptr::null_mut()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_closing(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_active(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_ref(_handle: *mut uv_handle_t) {}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_unref(_handle: *mut uv_handle_t) {}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_has_ref(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_set_data(
  handle: *mut uv_handle_t,
  data: *mut c_void,
) {
  if handle.is_null() {
    return;
  }
  unsafe {
    (*handle).data = data;
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_data(
  handle: *const uv_handle_t,
) -> *mut c_void {
  if handle.is_null() {
    return std::ptr::null_mut();
  }
  unsafe { (*handle).data }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_loop(
  handle: *const uv_handle_t,
) -> *mut uv_loop_t {
  if handle.is_null() {
    return std::ptr::null_mut();
  }
  unsafe { (*handle).r#loop }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_type(handle: *const uv_handle_t) -> c_int {
  if handle.is_null() {
    return uv_handle_type::UV_UNKNOWN_HANDLE as c_int;
  }
  unsafe { (*handle).r#type as c_int }
}

// uv_cpu_info: report no available CPU info. Callers (e.g. Sentry's
// profiler) treat this as a non-fatal degradation.
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_cpu_info(
  _cpu_infos: *mut *mut c_void,
  count: *mut c_int,
) -> c_int {
  if !count.is_null() {
    unsafe { *count = 0 };
  }
  // UV_ENOSYS (-libc::ENOSYS on unix); -4093 matches libuv's numbering for
  // ENOSYS on Linux. Any non-zero return signals "unsupported" to the addon.
  -4093
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_free_cpu_info(_cpu_infos: *mut c_void, _count: c_int) {
  // uv_cpu_info never allocates in our polyfill.
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
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_timer_t>(),
      UV_TIMER_SIZE
    );
    assert_eq!(std::mem::size_of::<uv_timer_t>(), UV_TIMER_SIZE);
  }
}
