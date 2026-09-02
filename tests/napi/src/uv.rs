// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(unix)]
use std::ffi::c_void;
#[cfg(unix)]
use std::fs::File;
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::fd::FromRawFd;
#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::ptr;
use std::ptr::addr_of_mut;
use std::ptr::null_mut;
#[cfg(unix)]
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use libuv_sys_lite::uv_async_init;
use libuv_sys_lite::uv_async_t;
use libuv_sys_lite::uv_close;
use libuv_sys_lite::uv_handle_t;
use libuv_sys_lite::uv_mutex_destroy;
use libuv_sys_lite::uv_mutex_lock;
use libuv_sys_lite::uv_mutex_t;
use libuv_sys_lite::uv_mutex_unlock;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

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
  unsafe {
    let handle = handle.cast::<uv_async_t>();
    let async_ = (*handle).data as *mut Async;
    let env = (*async_).env;
    assert_napi_ok!(napi_delete_reference(env, (*async_).callback));

    uv_mutex_destroy((*async_).mutex);
    let _ = Box::from_raw((*async_).mutex);
    let _ = Box::from_raw(async_);
    let _ = Box::from_raw(handle);
  }
}

unsafe extern "C" fn callback(handle: *mut uv_async_t) {
  unsafe {
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
}

unsafe fn uv_async_send(ptr: UvAsyncPtr) {
  assert_napi_ok!(libuv_sys_lite::uv_async_send(ptr.0));
}

fn make_uv_mutex() -> *mut uv_mutex_t {
  let mutex = new_raw(MaybeUninit::<uv_mutex_t>::uninit());
  assert_napi_ok!(libuv_sys_lite::uv_mutex_init(mutex.cast()));
  mutex.cast()
}

#[allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]
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

/// Test that uv_async_init keeps the event loop alive without any other
/// ref (no KeepAlive/threadsafe function). A worker thread fires
/// uv_async_send after a short delay; the callback prints a message and
/// closes the handle. Without proper ref-counting in uv_async_init/uv_close,
/// the process would exit before the callback fires.
unsafe extern "C" fn ref_callback(handle: *mut uv_async_t) {
  unsafe {
    let async_ = (*handle).data as *mut RefAsync;
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
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      0,
      ptr::null(),
      &mut result,
    ));
    assert_napi_ok!(napi_delete_reference(env, (*async_).callback));
    let _ = Box::from_raw(async_);
    uv_close(handle.cast(), Some(ref_close_cb));
  }
}

unsafe extern "C" fn ref_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let _ = Box::from_raw(handle.cast::<uv_async_t>());
  }
}

struct RefAsync {
  env: napi_env,
  callback: napi_ref,
}

#[allow(unused_unsafe, reason = "only unsafe on Windows")]
extern "C" fn test_uv_async_ref(
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

  let data = new_raw(RefAsync {
    env,
    callback: js_cb,
  });
  unsafe {
    addr_of_mut!((*uv_async).data).write(data.cast());
    assert_napi_ok!(uv_async_init(loop_.cast(), uv_async, Some(ref_callback)));
    let uv_async = UvAsyncPtr(uv_async);
    std::thread::spawn(move || {
      std::thread::sleep(Duration::from_millis(50));
      uv_async_send(uv_async);
    });
  }

  ptr::null_mut()
}

struct CloseAfterSendAsync {
  env: napi_env,
  callback: napi_ref,
}

unsafe extern "C" fn close_after_send_async_cb(_handle: *mut uv_async_t) {
  // This callback must not run once uv_close has marked the handle closing.
  // If it does, the queued callback retained and dereferenced a stale
  // addon-owned uv_async_t after the close callback was allowed to free it.
  std::process::abort();
}

unsafe extern "C" fn close_after_send_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let handle = handle.cast::<uv_async_t>();
    let async_ = (*handle).data as *mut CloseAfterSendAsync;
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
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      0,
      ptr::null(),
      &mut result,
    ));
    assert_napi_ok!(napi_delete_reference(env, (*async_).callback));
    let _ = Box::from_raw(async_);
    let _ = Box::from_raw(handle);
  }
}

#[allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]
extern "C" fn test_uv_async_close_after_send(
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

  let data = new_raw(CloseAfterSendAsync {
    env,
    callback: js_cb,
  });
  unsafe {
    addr_of_mut!((*uv_async).data).write(data.cast());
    assert_napi_ok!(uv_async_init(
      loop_.cast(),
      uv_async,
      Some(close_after_send_async_cb),
    ));
    assert_napi_ok!(libuv_sys_lite::uv_async_send(uv_async));
    uv_close(uv_async.cast(), Some(close_after_send_close_cb));
  }

  ptr::null_mut()
}

// Smoke test for the new uv polyfills (uv_hrtime, uv_timer_*, uv_cpu_info,
// uv_handle_*, uv_default_loop, uv_is_active/closing, uv_ref/unref). The
// goal is to verify that the symbols are exported from the host binary and
// behave like their libuv counterparts to the extent that the polyfills
// promise. Timer callbacks are bridged onto deno_core's uv_compat loop;
// here we synchronously start+stop the timer in the same napi callback so
// the event loop has no opportunity to fire it, matching the original
// no-op-stub-era assertion that the user callback is not invoked.
#[allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]
extern "C" fn test_uv_polyfills(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  use std::ffi::c_int;
  use std::mem::MaybeUninit;
  use std::mem::size_of;
  use std::ptr;
  use std::ptr::addr_of_mut;

  use libuv_sys_lite::uv_close;
  use libuv_sys_lite::uv_cpu_info;
  use libuv_sys_lite::uv_cpu_info_t;
  use libuv_sys_lite::uv_default_loop;
  use libuv_sys_lite::uv_free_cpu_info;
  use libuv_sys_lite::uv_handle_get_data;
  use libuv_sys_lite::uv_handle_set_data;
  use libuv_sys_lite::uv_handle_size;
  use libuv_sys_lite::uv_handle_t;
  use libuv_sys_lite::uv_handle_type;
  use libuv_sys_lite::uv_hrtime;
  use libuv_sys_lite::uv_is_active;
  use libuv_sys_lite::uv_is_closing;
  use libuv_sys_lite::uv_ref;
  use libuv_sys_lite::uv_strerror;
  use libuv_sys_lite::uv_timer_init;
  use libuv_sys_lite::uv_timer_set_repeat;
  use libuv_sys_lite::uv_timer_start;
  use libuv_sys_lite::uv_timer_stop;
  use libuv_sys_lite::uv_timer_t;
  use libuv_sys_lite::uv_unref;

  unsafe {
    // uv_hrtime must produce a monotonically non-decreasing value. Some
    // platforms can observe the timer origin on the first read.
    let t1 = uv_hrtime();
    let t2 = uv_hrtime();
    assert!(t2 >= t1);

    // uv_default_loop returns null (Deno does not expose a libuv loop
    // pointer to addons). uv_timer_init resolves the real backing loop
    // from a thread-local registered at op_napi_open time.
    let _loop = uv_default_loop();

    // uv_cpu_info reports unsupported (non-zero error). The Sentry profiler
    // checks the error code and skips CPU stats on failure.
    let mut cpu_infos: *mut uv_cpu_info_t = ptr::null_mut();
    let mut count: c_int = 42;
    let err = uv_cpu_info(&mut cpu_infos, &mut count);
    assert_ne!(err, 0);
    assert_eq!(count, 0);
    uv_free_cpu_info(cpu_infos, count);

    // uv_timer_init/start/stop must not crash. The user callback is started
    // and stopped synchronously here, so the event loop never has a chance
    // to dispatch it.
    let mut timer: MaybeUninit<uv_timer_t> = MaybeUninit::zeroed();
    assert_eq!(uv_timer_init(uv_default_loop(), timer.as_mut_ptr()), 0);
    let timer_ptr = timer.as_mut_ptr();

    unsafe extern "C" fn never_called(_handle: *mut uv_timer_t) {
      unreachable!("uv_timer was stopped synchronously before the loop polled");
    }
    assert_eq!(uv_timer_start(timer_ptr, Some(never_called), 1, 1), 0);
    uv_timer_set_repeat(timer_ptr, 1);
    assert_eq!(uv_timer_stop(timer_ptr), 0);

    // uv_handle_set_data/get_data round-trips on a stub handle.
    let handle = timer_ptr.cast::<uv_handle_t>();
    let cookie = 0x1234_5678usize as *mut std::ffi::c_void;
    uv_handle_set_data(handle, cookie);
    assert_eq!(uv_handle_get_data(handle), cookie);
    // restore for clean uv_close
    uv_handle_set_data(handle, ptr::null_mut());

    // uv_ref/uv_unref/uv_is_active/uv_is_closing should not crash on the
    // stub handle.
    uv_ref(handle);
    uv_unref(handle);
    let _ = uv_is_active(handle);
    let _ = uv_is_closing(handle);

    // uv_close on a stub timer with a null close_cb must not crash. The
    // polyfill's uv_close should detect UV_TIMER and skip a null callback.
    uv_close(handle, None);

    assert_eq!(
      uv_handle_size(uv_handle_type::UV_POLL),
      size_of::<libuv_sys_lite::uv_poll_t>()
    );
    assert_eq!(
      uv_handle_size(uv_handle_type::UV_UNKNOWN_HANDLE),
      usize::MAX
    );

    assert_eq!(std::ffi::CStr::from_ptr(uv_strerror(-4095)), c"end of file");
    assert_eq!(
      std::ffi::CStr::from_ptr(uv_strerror(0)),
      c"Unknown system error"
    );

    // Force the address of every export we care about so that link-time
    // resolution is exercised even if all earlier asserts were optimized
    // away.
    let _ = (
      uv_hrtime as *const () as usize,
      uv_default_loop as *const () as usize,
      uv_cpu_info as *const () as usize,
      uv_free_cpu_info as *const () as usize,
      uv_timer_init as *const () as usize,
      uv_timer_start as *const () as usize,
      uv_timer_stop as *const () as usize,
      uv_timer_set_repeat as *const () as usize,
      uv_handle_set_data as *const () as usize,
      uv_handle_get_data as *const () as usize,
      uv_ref as *const () as usize,
      uv_unref as *const () as usize,
      uv_is_active as *const () as usize,
      uv_is_closing as *const () as usize,
      uv_close as *const () as usize,
      uv_handle_size as *const () as usize,
      uv_strerror as *const () as usize,
    );

    // Touch addr_of_mut to silence unused import warnings on platforms
    // where the body above is fully elided.
    let _ = addr_of_mut!(count);
  }

  let mut undefined: napi_value = ptr::null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

// Verifies that a uv_timer scheduled by a NAPI addon actually fires on
// the deno event loop (i.e. the uv_compat bridge is wired up). The addon
// passes a JS callback that resolves the test promise from inside the
// libuv timer tick. The active timer holds an event-loop ref until it
// is closed; the JS callback ref and the heap allocations are released
// in the uv_close callback once the timer has fired.
struct TimerTestState {
  env: napi_env,
  callback: napi_ref,
  timer: *mut libuv_sys_lite::uv_timer_t,
}

unsafe extern "C" fn timer_test_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let state =
      libuv_sys_lite::uv_handle_get_data(handle) as *mut TimerTestState;
    if !state.is_null() {
      let env = (*state).env;
      assert_napi_ok!(napi_delete_reference(env, (*state).callback));
      let _ = Box::from_raw((*state).timer);
      let _ = Box::from_raw(state);
    }
  }
}

unsafe extern "C" fn timer_test_tick(handle: *mut libuv_sys_lite::uv_timer_t) {
  unsafe {
    use libuv_sys_lite::uv_close;
    use libuv_sys_lite::uv_handle_get_data;
    use libuv_sys_lite::uv_timer_stop;

    let state = uv_handle_get_data(handle.cast()) as *mut TimerTestState;
    let env = (*state).env;
    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      env,
      (*state).callback,
      &mut js_cb
    ));
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      0,
      ptr::null(),
      &mut result,
    ));
    // Stop and close so the second tick (1ms repeat) doesn't re-enter
    // after the test's JS promise has already resolved.
    uv_timer_stop(handle);
    uv_close(handle.cast(), Some(timer_test_close_cb));
  }
}

#[allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]
extern "C" fn test_uv_timer_fires(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  use libuv_sys_lite::uv_handle_set_data;
  use libuv_sys_lite::uv_timer_init;
  use libuv_sys_lite::uv_timer_start;
  use libuv_sys_lite::uv_timer_t;

  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  unsafe {
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
  }

  let timer = Box::into_raw(Box::new(MaybeUninit::<uv_timer_t>::zeroed()))
    as *mut uv_timer_t;
  let mut js_cb = null_mut();
  unsafe {
    assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  }
  let state = Box::into_raw(Box::new(TimerTestState {
    env,
    callback: js_cb,
    timer,
  }));
  unsafe {
    assert_napi_ok!(uv_timer_init(loop_.cast(), timer));
    uv_handle_set_data(timer.cast(), state.cast());
    assert_napi_ok!(uv_timer_start(timer, Some(timer_test_tick), 5, 1));
  }

  ptr::null_mut()
}

struct LoopHelperState {
  env: napi_env,
  callback: napi_ref,
  check: *mut libuv_sys_lite::uv_check_t,
  idle: *mut libuv_sys_lite::uv_idle_t,
  work: *mut libuv_sys_lite::uv_work_t,
  completed: u32,
  work_ran: std::sync::atomic::AtomicBool,
}

unsafe fn loop_helper_complete(state: *mut LoopHelperState) {
  unsafe {
    (*state).completed += 1;
    if (*state).completed != 2 {
      return;
    }
    assert!((*state).work_ran.load(std::sync::atomic::Ordering::Acquire));

    let env = (*state).env;
    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      env,
      (*state).callback,
      &mut js_cb
    ));
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      0,
      ptr::null(),
      &mut result,
    ));

    libuv_sys_lite::uv_check_stop((*state).check);
    assert_napi_ok!(napi_delete_reference(env, (*state).callback));
    let _ = Box::from_raw((*state).check);
    let _ = Box::from_raw((*state).idle);
    let _ = Box::from_raw((*state).work);
    let _ = Box::from_raw(state);
  }
}

unsafe extern "C" fn loop_helper_check_cb(
  check: *mut libuv_sys_lite::uv_check_t,
) {
  unsafe {
    let state =
      libuv_sys_lite::uv_handle_get_data(check.cast()) as *mut LoopHelperState;
    loop_helper_complete(state);
  }
}

unsafe extern "C" fn loop_helper_work_cb(work: *mut libuv_sys_lite::uv_work_t) {
  unsafe {
    let state = (*work).data as *mut LoopHelperState;
    (*state)
      .work_ran
      .store(true, std::sync::atomic::Ordering::Release);
  }
}

unsafe extern "C" fn loop_helper_after_work_cb(
  work: *mut libuv_sys_lite::uv_work_t,
  status: i32,
) {
  assert_eq!(status, 0);
  unsafe {
    let state = (*work).data as *mut LoopHelperState;
    loop_helper_complete(state);
  }
}

#[allow(unused_unsafe, reason = "napi_sys safe fn in unsafe extern blocks")]
extern "C" fn test_uv_loop_helpers(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  use libuv_sys_lite::uv_check_init;
  use libuv_sys_lite::uv_check_start;
  use libuv_sys_lite::uv_handle_set_data;
  use libuv_sys_lite::uv_idle_init;
  use libuv_sys_lite::uv_idle_start;
  use libuv_sys_lite::uv_os_getpid;
  use libuv_sys_lite::uv_queue_work;

  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  unsafe {
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
  }

  let check = Box::into_raw(Box::new(
    MaybeUninit::<libuv_sys_lite::uv_check_t>::zeroed(),
  )) as *mut libuv_sys_lite::uv_check_t;
  let idle =
    Box::into_raw(Box::new(MaybeUninit::<libuv_sys_lite::uv_idle_t>::zeroed()))
      as *mut libuv_sys_lite::uv_idle_t;
  let work =
    Box::into_raw(Box::new(MaybeUninit::<libuv_sys_lite::uv_work_t>::zeroed()))
      as *mut libuv_sys_lite::uv_work_t;

  let mut js_cb = null_mut();
  unsafe {
    assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  }
  let state = Box::into_raw(Box::new(LoopHelperState {
    env,
    callback: js_cb,
    check,
    idle,
    work,
    completed: 0,
    work_ran: std::sync::atomic::AtomicBool::new(false),
  }));

  unsafe {
    assert!(uv_os_getpid() > 0);
    assert_napi_ok!(uv_check_init(loop_.cast(), check));
    uv_handle_set_data(check.cast(), state.cast());
    assert_napi_ok!(uv_idle_init(loop_.cast(), idle));
    assert_napi_ok!(uv_idle_start(idle, None));
    (*work).data = state.cast();
    assert_napi_ok!(uv_queue_work(
      loop_.cast(),
      work,
      Some(loop_helper_work_cb),
      Some(loop_helper_after_work_cb),
    ));
    assert_napi_ok!(uv_check_start(check, Some(loop_helper_check_cb)));
  }

  ptr::null_mut()
}

// Exercises the libuv threading + semaphore polyfills (uv_thread_*,
// uv_sem_*) added to the host binary in ext/napi/uv.rs. Like the other
// uv_* symbols in this file, they are resolved from the host `deno`
// process at runtime by libuv-sys-lite (dyn-symbols) — declaring them
// directly would create static imports that fail to link on Windows. A
// worker thread increments a counter and posts a counting semaphore three
// times; the main thread drains the semaphore, joins the worker, and
// checks the results.
struct ThreadArg {
  sem: *mut libuv_sys_lite::uv_sem_t,
  counter: *mut i32,
}

unsafe extern "C" fn uv_threads_entry(arg: *mut std::ffi::c_void) {
  unsafe {
    let a = arg as *mut ThreadArg;
    for _ in 0..3 {
      *(*a).counter += 1;
      libuv_sys_lite::uv_sem_post((*a).sem);
    }
  }
}

extern "C" fn test_uv_threads(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  use libuv_sys_lite::uv_sem_destroy;
  use libuv_sys_lite::uv_sem_init;
  use libuv_sys_lite::uv_sem_t;
  use libuv_sys_lite::uv_sem_trywait;
  use libuv_sys_lite::uv_sem_wait;
  use libuv_sys_lite::uv_thread_create;
  use libuv_sys_lite::uv_thread_equal;
  use libuv_sys_lite::uv_thread_join;
  use libuv_sys_lite::uv_thread_self;
  use libuv_sys_lite::uv_thread_t;

  unsafe {
    let mut sem = MaybeUninit::<uv_sem_t>::zeroed();
    let sem_ptr = sem.as_mut_ptr();
    assert_eq!(uv_sem_init(sem_ptr, 0), 0);

    let mut counter: i32 = 0;
    let mut arg = ThreadArg {
      sem: sem_ptr,
      counter: &mut counter,
    };
    let arg_ptr: *mut ThreadArg = &mut arg;

    let mut tid = MaybeUninit::<uv_thread_t>::zeroed();
    let tid_ptr = tid.as_mut_ptr();
    assert_eq!(
      uv_thread_create(tid_ptr, Some(uv_threads_entry), arg_ptr.cast()),
      0
    );

    // Drain the three posts from the worker (blocks until they arrive).
    for _ in 0..3 {
      uv_sem_wait(sem_ptr);
    }
    assert_eq!(uv_thread_join(tid_ptr), 0);
    assert_eq!(counter, 3);

    // The count is back to zero, so a non-blocking wait must fail.
    assert_ne!(uv_sem_trywait(sem_ptr), 0);

    // uv_thread_self / uv_thread_equal smoke check.
    let _ = uv_thread_self();
    assert_ne!(uv_thread_equal(tid_ptr, tid_ptr), 0);

    uv_sem_destroy(sem_ptr);
  }

  let mut undefined: napi_value = ptr::null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

// Exercises the libuv condition-variable polyfills (uv_cond_*) added to the
// host binary in ext/napi/uv.rs, resolved from the host `deno` process at
// runtime like the other uv_* symbols here. The main thread waits on a
// condition variable until a worker sets a predicate (guarded by the mutex)
// and signals it; uv_cond_timedwait is then checked to report UV_ETIMEDOUT
// when nobody signals.
struct CondArg {
  mutex: *mut libuv_sys_lite::uv_mutex_t,
  cond: *mut libuv_sys_lite::uv_cond_t,
  ready: *mut bool,
}

unsafe extern "C" fn uv_cond_entry(arg: *mut std::ffi::c_void) {
  unsafe {
    let a = arg as *mut CondArg;
    libuv_sys_lite::uv_mutex_lock((*a).mutex);
    *(*a).ready = true;
    libuv_sys_lite::uv_cond_signal((*a).cond);
    libuv_sys_lite::uv_mutex_unlock((*a).mutex);
  }
}

extern "C" fn test_uv_cond(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  use libuv_sys_lite::uv_cond_destroy;
  use libuv_sys_lite::uv_cond_init;
  use libuv_sys_lite::uv_cond_t;
  use libuv_sys_lite::uv_cond_timedwait;
  use libuv_sys_lite::uv_cond_wait;
  use libuv_sys_lite::uv_mutex_destroy;
  use libuv_sys_lite::uv_mutex_init;
  use libuv_sys_lite::uv_mutex_lock;
  use libuv_sys_lite::uv_mutex_t;
  use libuv_sys_lite::uv_mutex_unlock;
  use libuv_sys_lite::uv_thread_create;
  use libuv_sys_lite::uv_thread_join;
  use libuv_sys_lite::uv_thread_t;

  unsafe {
    let mut mutex = MaybeUninit::<uv_mutex_t>::zeroed();
    let mutex_ptr = mutex.as_mut_ptr();
    assert_eq!(uv_mutex_init(mutex_ptr), 0);
    let mut cond = MaybeUninit::<uv_cond_t>::zeroed();
    let cond_ptr = cond.as_mut_ptr();
    assert_eq!(uv_cond_init(cond_ptr), 0);

    let mut ready = false;
    let mut arg = CondArg {
      mutex: mutex_ptr,
      cond: cond_ptr,
      ready: &mut ready,
    };
    let arg_ptr: *mut CondArg = &mut arg;

    // Hold the mutex, then spawn the worker — it blocks on the mutex until
    // uv_cond_wait below releases it.
    uv_mutex_lock(mutex_ptr);
    let mut tid = MaybeUninit::<uv_thread_t>::zeroed();
    let tid_ptr = tid.as_mut_ptr();
    assert_eq!(
      uv_thread_create(tid_ptr, Some(uv_cond_entry), arg_ptr.cast()),
      0
    );
    while !ready {
      uv_cond_wait(cond_ptr, mutex_ptr);
    }
    uv_mutex_unlock(mutex_ptr);
    assert_eq!(uv_thread_join(tid_ptr), 0);
    assert!(ready);

    // With nobody signaling, uv_cond_timedwait must report the platform's
    // UV_ETIMEDOUT (the value the addon itself is compiled against), not just a
    // non-zero code. Loop to tolerate spurious (rc == 0) wakeups.
    let uv_etimedout = libuv_sys_lite::uv_errno_t::UV_ETIMEDOUT.0;
    let start = Instant::now();
    let rc = loop {
      uv_mutex_lock(mutex_ptr);
      let rc = uv_cond_timedwait(cond_ptr, mutex_ptr, 5_000_000);
      uv_mutex_unlock(mutex_ptr);
      if rc != 0 {
        break rc;
      }
      assert!(start.elapsed() < Duration::from_secs(5));
    };
    assert_eq!(rc, uv_etimedout);

    uv_cond_destroy(cond_ptr);
    uv_mutex_destroy(mutex_ptr);
  }

  let mut undefined: napi_value = ptr::null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

// Exercises uv_cond_broadcast against multiple parked waiters (the
// single-waiter/uv_cond_signal path is covered by test_uv_cond above). Several
// worker threads each block in uv_cond_wait on the same condition variable;
// once all of them are parked the main thread flips the predicate and wakes
// every one of them with a single uv_cond_broadcast.
struct BroadcastArg {
  mutex: *mut libuv_sys_lite::uv_mutex_t,
  cond: *mut libuv_sys_lite::uv_cond_t,
  go: *mut bool,
  waiting: *mut i32,
  woken: *mut i32,
}

unsafe extern "C" fn uv_cond_broadcast_entry(arg: *mut std::ffi::c_void) {
  unsafe {
    let a = arg as *mut BroadcastArg;
    libuv_sys_lite::uv_mutex_lock((*a).mutex);
    // Announce we're about to park, then wait for the predicate. The
    // increment happens under the mutex, so once main observes `waiting == N`
    // every worker has released the mutex inside uv_cond_wait and is parked.
    *(*a).waiting += 1;
    while !*(*a).go {
      libuv_sys_lite::uv_cond_wait((*a).cond, (*a).mutex);
    }
    *(*a).woken += 1;
    libuv_sys_lite::uv_mutex_unlock((*a).mutex);
  }
}

extern "C" fn test_uv_cond_broadcast(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  use libuv_sys_lite::uv_cond_broadcast;
  use libuv_sys_lite::uv_cond_destroy;
  use libuv_sys_lite::uv_cond_init;
  use libuv_sys_lite::uv_cond_t;
  use libuv_sys_lite::uv_mutex_destroy;
  use libuv_sys_lite::uv_mutex_init;
  use libuv_sys_lite::uv_mutex_lock;
  use libuv_sys_lite::uv_mutex_t;
  use libuv_sys_lite::uv_mutex_unlock;
  use libuv_sys_lite::uv_thread_create;
  use libuv_sys_lite::uv_thread_join;
  use libuv_sys_lite::uv_thread_t;

  const N: i32 = 4;

  unsafe {
    let mut mutex = MaybeUninit::<uv_mutex_t>::zeroed();
    let mutex_ptr = mutex.as_mut_ptr();
    assert_eq!(uv_mutex_init(mutex_ptr), 0);
    let mut cond = MaybeUninit::<uv_cond_t>::zeroed();
    let cond_ptr = cond.as_mut_ptr();
    assert_eq!(uv_cond_init(cond_ptr), 0);

    let mut go = false;
    let mut waiting = 0;
    let mut woken = 0;
    let mut arg = BroadcastArg {
      mutex: mutex_ptr,
      cond: cond_ptr,
      go: &mut go,
      waiting: &mut waiting,
      woken: &mut woken,
    };
    let arg_ptr: *mut BroadcastArg = &mut arg;

    let mut tids = [MaybeUninit::<uv_thread_t>::zeroed(); N as usize];
    for tid in &mut tids {
      assert_eq!(
        uv_thread_create(
          tid.as_mut_ptr(),
          Some(uv_cond_broadcast_entry),
          arg_ptr.cast(),
        ),
        0
      );
    }

    // Wait until every worker is parked in uv_cond_wait before broadcasting, so
    // the single broadcast below is what releases all of them.
    let start = Instant::now();
    loop {
      uv_mutex_lock(mutex_ptr);
      let parked = waiting;
      uv_mutex_unlock(mutex_ptr);
      if parked == N {
        break;
      }
      assert!(start.elapsed() < Duration::from_secs(5));
      std::thread::sleep(Duration::from_millis(1));
    }

    // One broadcast wakes all parked waiters. Set the predicate through the
    // shared pointer (same memory as `go`) under the mutex, mirroring how the
    // workers observe it.
    uv_mutex_lock(mutex_ptr);
    *arg.go = true;
    uv_cond_broadcast(cond_ptr);
    uv_mutex_unlock(mutex_ptr);

    for tid in &mut tids {
      assert_eq!(uv_thread_join(tid.as_mut_ptr()), 0);
    }
    assert_eq!(woken, N);

    uv_cond_destroy(cond_ptr);
    uv_mutex_destroy(mutex_ptr);
  }

  let mut undefined: napi_value = ptr::null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

#[cfg(unix)]
extern "C" fn test_uv_poll_init_sets_nonblocking(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  unsafe {
    let mut loop_ = null_mut();
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
    let mut fds = [0; 2];
    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
    let poll_fd = OwnedFd::from_raw_fd(fds[0]);
    let _other_fd = OwnedFd::from_raw_fd(fds[1]);
    let flags = libc::fcntl(poll_fd.as_raw_fd(), libc::F_GETFL);
    assert_ne!(flags, -1);
    assert_eq!(flags & libc::O_NONBLOCK, 0);

    let mut poll = MaybeUninit::<libuv_sys_lite::uv_poll_t>::zeroed();
    assert_eq!(
      libuv_sys_lite::uv_poll_init(
        loop_.cast(),
        poll.as_mut_ptr(),
        poll_fd.as_raw_fd(),
      ),
      0
    );
    let flags = libc::fcntl(poll_fd.as_raw_fd(), libc::F_GETFL);
    assert_ne!(flags, -1);
    assert_ne!(flags & libc::O_NONBLOCK, 0);
    libuv_sys_lite::uv_close(poll.as_mut_ptr().cast(), None);
  }

  let mut undefined = null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

#[cfg(unix)]
struct PollTest {
  env: napi_env,
  // Keeps the JavaScript completion callback alive across asynchronous work.
  callback: napi_ref,
  poll: *mut libuv_sys_lite::uv_poll_t,
  // Starts as the watchdog; unrelated fixtures may reuse it for a settle
  // window after first stopping it.
  timer: *mut libuv_sys_lite::uv_timer_t,
  poll_fd: OwnedFd,
  other_fd: OwnedFd,
  poll_callback_count: usize,
  closing: bool,
  poll_closed: bool,
  passed: bool,
  closed_handles: usize,
  expected_status: i32,
  expected_events: i32,
  expected_active: bool,
  settle_after_callback: bool,
}

#[cfg(unix)]
struct MultiPollTest {
  env: napi_env,
  callback: napi_ref,
  first_poll: *mut libuv_sys_lite::uv_poll_t,
  second_poll: *mut libuv_sys_lite::uv_poll_t,
  timer: *mut libuv_sys_lite::uv_timer_t,
  first_reader: OwnedFd,
  first_writer: OwnedFd,
  second_reader: OwnedFd,
  // Keeps the helper's raw descriptor valid until its rendezvous write and
  // acknowledgement complete during the first callback.
  _second_writer: OwnedFd,
  first_ready: mpsc::SyncSender<()>,
  second_write_result: mpsc::Receiver<isize>,
  first_callback_count: usize,
  second_callback_count: usize,
  first_callback_returned: bool,
  closing: bool,
  passed: bool,
  closed_handles: usize,
}

#[cfg(unix)]
unsafe fn multi_poll_test_finish(state: *mut MultiPollTest, passed: bool) {
  unsafe {
    // Either poll callback or the timeout may finish first. Once closing has
    // begun, retain a late failure without scheduling duplicate closes.
    if (*state).closing {
      (*state).passed &= passed;
      return;
    }
    (*state).closing = true;
    (*state).passed &= passed;
    assert_eq!(libuv_sys_lite::uv_poll_stop((*state).first_poll), 0);
    assert_eq!(libuv_sys_lite::uv_poll_stop((*state).second_poll), 0);
    assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
    libuv_sys_lite::uv_close(
      (*state).first_poll.cast(),
      Some(multi_poll_test_close_cb),
    );
    libuv_sys_lite::uv_close(
      (*state).second_poll.cast(),
      Some(multi_poll_test_close_cb),
    );
    libuv_sys_lite::uv_close(
      (*state).timer.cast(),
      Some(multi_poll_test_close_cb),
    );
  }
}

#[cfg(unix)]
unsafe extern "C" fn multi_poll_test_close_cb(
  handle: *mut libuv_sys_lite::uv_handle_t,
) {
  unsafe {
    let state = (*handle).data.cast::<MultiPollTest>();
    (*state).closed_handles += 1;
    // libuv may invoke these three close callbacks in any order. The last one
    // owns reporting the result and freeing state after every handle closes.
    if (*state).closed_handles != 3 {
      return;
    }

    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      (*state).env,
      (*state).callback,
      &mut js_cb
    ));
    let mut global = null_mut();
    assert_napi_ok!(napi_get_global((*state).env, &mut global));
    let mut passed = null_mut();
    assert_napi_ok!(napi_get_boolean(
      (*state).env,
      (*state).passed,
      &mut passed
    ));
    let mut result = null_mut();
    assert_napi_ok!(napi_call_function(
      (*state).env,
      global,
      js_cb,
      1,
      &passed,
      &mut result,
    ));
    assert_napi_ok!(napi_delete_reference((*state).env, (*state).callback));

    let _ = Box::from_raw((*state).first_poll);
    let _ = Box::from_raw((*state).second_poll);
    let _ = Box::from_raw((*state).timer);
    let _ = Box::from_raw(state);
  }
}

#[cfg(unix)]
unsafe extern "C" fn multi_poll_test_timeout(
  timer: *mut libuv_sys_lite::uv_timer_t,
) {
  unsafe {
    multi_poll_test_finish((*timer).data.cast(), false);
  }
}

#[cfg(unix)]
unsafe fn new_multi_poll_test(
  env: napi_env,
  callback: napi_value,
) -> *mut MultiPollTest {
  unsafe {
    let mut first_fds = [0; 2];
    let mut second_fds = [0; 2];
    assert_eq!(libc::pipe(first_fds.as_mut_ptr()), 0);
    assert_eq!(libc::pipe(second_fds.as_mut_ptr()), 0);
    let first_reader = OwnedFd::from_raw_fd(first_fds[0]);
    let first_writer = OwnedFd::from_raw_fd(first_fds[1]);
    let second_reader = OwnedFd::from_raw_fd(second_fds[0]);
    let second_writer = OwnedFd::from_raw_fd(second_fds[1]);
    let (first_ready, wait_for_first) = mpsc::sync_channel(0);
    let (second_was_written, second_write_result) = mpsc::sync_channel(1);
    let second_writer_fd = second_writer.as_raw_fd();
    std::thread::spawn(move || {
      if wait_for_first.recv().is_ok() {
        let written = libc::write(second_writer_fd, b"x".as_ptr().cast(), 1);
        let _ = second_was_written.send(written);
      }
    });

    let first_poll = Box::into_raw(Box::new(MaybeUninit::<
      libuv_sys_lite::uv_poll_t,
    >::zeroed()))
    .cast();
    let second_poll = Box::into_raw(Box::new(MaybeUninit::<
      libuv_sys_lite::uv_poll_t,
    >::zeroed()))
    .cast();
    let timer = Box::into_raw(Box::new(MaybeUninit::<
      libuv_sys_lite::uv_timer_t,
    >::zeroed()))
    .cast();
    let mut loop_ = null_mut();
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
    let mut callback_ref = null_mut();
    assert_napi_ok!(napi_create_reference(env, callback, 1, &mut callback_ref));
    let state = Box::into_raw(Box::new(MultiPollTest {
      env,
      callback: callback_ref,
      first_poll,
      second_poll,
      timer,
      first_reader,
      first_writer,
      second_reader,
      _second_writer: second_writer,
      first_ready,
      second_write_result,
      first_callback_count: 0,
      second_callback_count: 0,
      first_callback_returned: false,
      closing: false,
      passed: true,
      closed_handles: 0,
    }));

    assert_eq!(
      libuv_sys_lite::uv_poll_init(
        loop_.cast(),
        first_poll,
        (*state).first_reader.as_raw_fd(),
      ),
      0
    );
    assert_eq!(
      libuv_sys_lite::uv_poll_init(
        loop_.cast(),
        second_poll,
        (*state).second_reader.as_raw_fd(),
      ),
      0
    );
    assert_eq!(libuv_sys_lite::uv_timer_init(loop_.cast(), timer), 0);
    libuv_sys_lite::uv_handle_set_data(first_poll.cast(), state.cast());
    libuv_sys_lite::uv_handle_set_data(second_poll.cast(), state.cast());
    libuv_sys_lite::uv_handle_set_data(timer.cast(), state.cast());
    assert_eq!(
      libuv_sys_lite::uv_timer_start(
        timer,
        Some(multi_poll_test_timeout),
        10_000,
        0,
      ),
      0
    );
    state
  }
}

#[cfg(unix)]
unsafe fn poll_test_finish(state: *mut PollTest, passed: bool) {
  unsafe {
    // All exit paths converge here so the watchdog and ordinary assertions use
    // the same idempotent cleanup sequence.
    if (*state).closing {
      // A callback after cleanup has begun is a test failure. Preserve the
      // failure without closing either handle twice.
      (*state).passed &= passed;
      return;
    }
    (*state).closing = true;
    (*state).passed &= passed;
    if !(*state).poll_closed {
      assert_eq!(libuv_sys_lite::uv_poll_stop((*state).poll), 0);
    }
    assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
    if !(*state).poll_closed {
      (*state).poll_closed = true;
      libuv_sys_lite::uv_close((*state).poll.cast(), Some(poll_test_close_cb));
    }
    libuv_sys_lite::uv_close((*state).timer.cast(), Some(poll_test_close_cb));
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_test_close_cb(
  handle: *mut libuv_sys_lite::uv_handle_t,
) {
  unsafe {
    let state = (*handle).data.cast::<PollTest>();
    (*state).closed_handles += 1;
    if (*state).closed_handles != 2 {
      return;
    }

    // Free shared state and resolve JavaScript only after both close callbacks
    // have run.
    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      (*state).env,
      (*state).callback,
      &mut js_cb
    ));
    let mut global = null_mut();
    assert_napi_ok!(napi_get_global((*state).env, &mut global));
    let mut passed = null_mut();
    assert_napi_ok!(napi_get_boolean(
      (*state).env,
      (*state).passed,
      &mut passed
    ));
    let mut result = null_mut();
    assert_napi_ok!(napi_call_function(
      (*state).env,
      global,
      js_cb,
      1,
      &passed,
      &mut result,
    ));
    assert_napi_ok!(napi_delete_reference((*state).env, (*state).callback));

    let _ = Box::from_raw((*state).poll);
    let _ = Box::from_raw((*state).timer);
    let _ = Box::from_raw(state);
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_test_timeout(timer: *mut libuv_sys_lite::uv_timer_t) {
  unsafe {
    let state = (*timer).data.cast::<PollTest>();
    poll_test_finish(state, false);
  }
}

#[cfg(unix)]
unsafe fn new_poll_test(env: napi_env, callback: napi_value) -> *mut PollTest {
  unsafe {
    // Create the usual pipe fixture: poll its read end and write to its peer.
    let mut fds = [0; 2];
    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
    let reader = OwnedFd::from_raw_fd(fds[0]);
    let writer = OwnedFd::from_raw_fd(fds[1]);
    new_poll_test_with_fds(env, callback, reader, writer)
  }
}

#[cfg(unix)]
struct ActivePollForTeardown {
  poll: *mut libuv_sys_lite::uv_poll_t,
  _reader: OwnedFd,
  _writer: OwnedFd,
}

#[cfg(unix)]
unsafe extern "C" fn active_poll_for_teardown_callback(
  _poll: *mut libuv_sys_lite::uv_poll_t,
  _status: i32,
  _events: i32,
) {
}

#[cfg(unix)]
unsafe extern "C" fn active_poll_for_teardown_cleanup(data: *mut c_void) {
  unsafe {
    // NapiState::drop invalidates this Env's poll scope before calling cleanup
    // hooks, so queued poll readiness cannot reach this bridge after the
    // addon-owned poll storage is released below.
    let state = Box::from_raw(data.cast::<ActivePollForTeardown>());
    libuv_sys_lite::uv_close(state.poll.cast(), None);
    let _ = Box::from_raw(state.poll);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_leave_active(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  unsafe {
    let mut unref = false;
    assert_napi_ok!(napi_get_value_bool(env, args[0], &mut unref));

    let mut fds = [0; 2];
    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
    let reader = OwnedFd::from_raw_fd(fds[0]);
    let writer = OwnedFd::from_raw_fd(fds[1]);
    let poll = Box::into_raw(Box::new(
      MaybeUninit::<libuv_sys_lite::uv_poll_t>::zeroed(),
    ))
    .cast();
    let state = Box::into_raw(Box::new(ActivePollForTeardown {
      poll,
      _reader: reader,
      _writer: writer,
    }));
    let mut loop_ = null_mut();
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
    assert_eq!(
      libuv_sys_lite::uv_poll_init(
        loop_.cast(),
        poll,
        (*state)._reader.as_raw_fd(),
      ),
      0
    );
    libuv_sys_lite::uv_handle_set_data(poll.cast(), state.cast());
    assert_eq!(
      libuv_sys_lite::uv_poll_start(
        poll,
        libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
        Some(active_poll_for_teardown_callback),
      ),
      0
    );
    // Exercise teardown both with and without this active poll keeping the
    // loop alive.
    if unref {
      libuv_sys_lite::uv_unref(poll.cast());
    }
    // Leave this ready poll and its storage to the environment cleanup hook.
    assert_eq!(
      libc::write((*state)._writer.as_raw_fd(), b"x".as_ptr().cast(), 1),
      1
    );
    assert_napi_ok!(napi_add_env_cleanup_hook(
      env,
      Some(active_poll_for_teardown_cleanup),
      state.cast(),
    ));
  }

  null_mut()
}

#[cfg(unix)]
unsafe fn new_poll_test_with_fds(
  env: napi_env,
  callback: napi_value,
  poll_fd: OwnedFd,
  other_fd: OwnedFd,
) -> *mut PollTest {
  unsafe {
    // The fixture owns both descriptors until its two libuv handles finish
    // closing, including for tests that provide sockets instead of a pipe.
    let poll = Box::into_raw(Box::new(
      MaybeUninit::<libuv_sys_lite::uv_poll_t>::zeroed(),
    ))
    .cast();
    let timer = Box::into_raw(Box::new(MaybeUninit::<
      libuv_sys_lite::uv_timer_t,
    >::zeroed()))
    .cast();
    let mut callback_ref = null_mut();
    assert_napi_ok!(napi_create_reference(env, callback, 1, &mut callback_ref));
    let mut loop_ = null_mut();
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

    let state = Box::into_raw(Box::new(PollTest {
      env,
      callback: callback_ref,
      poll,
      timer,
      poll_fd,
      other_fd,
      poll_callback_count: 0,
      closing: false,
      poll_closed: false,
      passed: true,
      closed_handles: 0,
      expected_status: 0,
      expected_events: 0,
      expected_active: false,
      settle_after_callback: false,
    }));

    assert_eq!(
      libuv_sys_lite::uv_poll_init(
        loop_.cast(),
        poll,
        (*state).poll_fd.as_raw_fd()
      ),
      0
    );
    assert_eq!(libuv_sys_lite::uv_timer_init(loop_.cast(), timer), 0);
    libuv_sys_lite::uv_handle_set_data(poll.cast(), state.cast());
    libuv_sys_lite::uv_handle_set_data(timer.cast(), state.cast());
    assert_eq!(
      // This failure-only watchdog leaves the short settle timers below
      // unchanged while providing enough margin for loaded CI workers.
      libuv_sys_lite::uv_timer_start(timer, Some(poll_test_timeout), 10_000, 0),
      0
    );
    state
  }
}

#[cfg(unix)]
unsafe fn poll_test_start(
  state: *mut PollTest,
  events: i32,
  callback: libuv_sys_lite::uv_poll_cb,
) {
  unsafe {
    assert_eq!(
      libuv_sys_lite::uv_poll_start((*state).poll, events, callback),
      0
    );
  }
}

#[cfg(unix)]
unsafe fn poll_test_write(state: *mut PollTest) {
  unsafe {
    assert_eq!(
      libc::write((*state).other_fd.as_raw_fd(), b"x".as_ptr().cast(), 1),
      1
    );
  }
}

#[cfg(unix)]
unsafe fn poll_test_start_timer(
  state: *mut PollTest,
  callback: libuv_sys_lite::uv_timer_cb,
  timeout: u64,
) {
  unsafe {
    // Reuse the watchdog timer for short post-condition windows after first
    // stopping it; this avoids introducing another handle to clean up.
    assert_eq!(
      libuv_sys_lite::uv_timer_start((*state).timer, callback, timeout, 0),
      0
    );
  }
}

#[cfg(unix)]
unsafe fn poll_test_expect(
  state: *mut PollTest,
  status: i32,
  events: i32,
  active: bool,
  settle_after_callback: bool,
) {
  unsafe {
    (*state).expected_status = status;
    (*state).expected_events = events;
    (*state).expected_active = active;
    (*state).settle_after_callback = settle_after_callback;
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_expected_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    (*state).poll_callback_count += 1;
    let active = libuv_sys_lite::uv_is_active(poll.cast()) != 0;
    (*state).passed &= (*state).poll_callback_count == 1
      && status == (*state).expected_status
      && events == (*state).expected_events
      && (*state).expected_active == active;

    if (*state).settle_after_callback {
      assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
      poll_test_start_timer(state, Some(poll_stop_settle_cb), 100);
    } else {
      poll_test_finish(state, (*state).passed);
    }
  }
}

#[cfg(unix)]
fn poll_callback_arg(env: napi_env, info: napi_callback_info) -> napi_value {
  // Poll tests receive one `done` Promise resolver, which the fixture retains
  // until asynchronous cleanup has completed.
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);
  args[0]
}

#[cfg(unix)]
extern "C" fn test_uv_poll_reports_actual_writable_events(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let mut fds = [0; 2];
    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
    let reader = OwnedFd::from_raw_fd(fds[0]);
    let writer = OwnedFd::from_raw_fd(fds[1]);
    let state =
      new_poll_test_with_fds(env, poll_callback_arg(env, info), writer, reader);
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    let writable = libuv_sys_lite::uv_poll_event::UV_WRITABLE.0 as i32;
    // A pipe's write end produces POLLOUT but not POLLIN. Requesting both bits
    // distinguishes actual readiness from the requested mask. Expecting
    // UV_WRITABLE (2) verifies that POLLOUT (4 on supported Unix targets) is
    // translated to the libuv value.
    poll_test_expect(state, 0, writable, true, false);
    poll_test_start(state, readable | writable, Some(poll_expected_cb));
  }
  null_mut()
}

#[cfg(unix)]
extern "C" fn test_uv_poll_dispatches_hangup_only(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let mut fds = [0; 2];
    assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
    let reader = OwnedFd::from_raw_fd(fds[0]);
    let writer = OwnedFd::from_raw_fd(fds[1]);
    drop(writer);
    // poll(2) reports POLLHUP independently of the requested mask. libuv
    // surfaces the requested readable interest so the consumer can observe EOF.
    let placeholder: OwnedFd = File::open("/dev/null").unwrap().into();
    let state = new_poll_test_with_fds(
      env,
      poll_callback_arg(env, info),
      reader,
      placeholder,
    );
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    poll_test_expect(state, 0, readable, true, false);
    poll_test_start(state, readable, Some(poll_expected_cb));
  }
  null_mut()
}

#[cfg(target_os = "linux")]
extern "C" fn test_uv_poll_reports_disconnect(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let mut fds = [0; 2];
    assert_eq!(
      libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr(),),
      0
    );
    let poll_socket = OwnedFd::from_raw_fd(fds[0]);
    let peer_socket = OwnedFd::from_raw_fd(fds[1]);
    let state = new_poll_test_with_fds(
      env,
      poll_callback_arg(env, info),
      poll_socket,
      peer_socket,
    );
    let disconnect = libuv_sys_lite::uv_poll_event::UV_DISCONNECT.0 as i32;
    poll_test_expect(state, 0, disconnect, true, false);
    poll_test_start(state, disconnect, Some(poll_expected_cb));

    // Linux reports POLLRDHUP when the peer closes its write side. Other Unix
    // poll backends do not guarantee an equivalent raw event, so the JavaScript
    // test runs this production-path assertion on Linux only.
    assert_eq!(
      libc::shutdown((*state).other_fd.as_raw_fd(), libc::SHUT_WR),
      0
    );
  }
  null_mut()
}

#[cfg(not(target_os = "linux"))]
extern "C" fn test_uv_poll_reports_disconnect(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut undefined = null_mut();
  unsafe {
    assert_napi_ok!(napi_get_undefined(env, &mut undefined));
  }
  undefined
}

#[cfg(unix)]
extern "C" fn test_uv_poll_invalid_fd_reports_ebadf(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    // Closing an fd after uv_poll_init is invalid libuv usage. Real libuv may
    // abort on Linux rather than deliver a callback, but poll(2) can return
    // POLLNVAL here. This compatibility layer defensively reports it once as
    // UV_EBADF and stops.
    //
    // Replace the reader before closing it so cleanup cannot close a later
    // descriptor that reuses the same fd.
    let placeholder: OwnedFd = File::open("/dev/null").unwrap().into();
    drop(std::mem::replace(&mut (*state).poll_fd, placeholder));
    poll_test_expect(
      state,
      libuv_sys_lite::uv_errno_t::UV_EBADF.0,
      0,
      false,
      false,
    );
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_expected_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_level_triggered_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).passed &= status == 0 && events == readable;
    (*state).poll_callback_count += 1;
    // Leave the byte unread. libuv polling is level-triggered, so the same
    // readiness must produce another callback without another uv_poll_start().
    if (*state).poll_callback_count == 2 {
      poll_test_finish(state, (*state).passed);
    }
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_repeats_while_readable(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_level_triggered_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_stop_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  _status: i32,
  _events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    // If this callback runs after uv_poll_stop(), cancellation failed.
    (*state).passed = false;
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_stop_settle_cb(
  timer: *mut libuv_sys_lite::uv_timer_t,
) {
  unsafe {
    poll_test_finish((*timer).data.cast(), true);
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_arm_then_stop_cb(
  timer: *mut libuv_sys_lite::uv_timer_t,
) {
  unsafe {
    let state = (*timer).data.cast::<PollTest>();
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_stop_cb),
    );
    // Keep the loop thread occupied long enough for the poll worker to observe
    // readiness and queue its callback. The task queue exposes no test-visible
    // latch, so this generous scheduling window is the closest public-API
    // approximation of a pending callback. stop must suppress that callback.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(libuv_sys_lite::uv_poll_stop((*state).poll), 0);
    poll_test_start_timer(state, Some(poll_stop_settle_cb), 100);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_stop_suppresses_ready_callback(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
    poll_test_start_timer(state, Some(poll_arm_then_stop_cb), 0);
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_no_flood_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    (*state).poll_callback_count += 1;
    if (*state).poll_callback_count != 1 {
      // Leave cleanup to the settle timer installed by the first callback.
      // Several callbacks may already be queued by the implementation under
      // test, so freeing shared state here would make the test itself racy.
      (*state).passed = false;
      assert_eq!(libuv_sys_lite::uv_poll_stop(poll), 0);
      return;
    }

    // Keep the loop-thread callback busy while the fd remains level-ready.
    // A poll worker may not enqueue successors until this callback returns.
    std::thread::sleep(Duration::from_millis(100));
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).passed = status == 0 && events == readable;
    assert_eq!(libuv_sys_lite::uv_poll_stop(poll), 0);
    assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
    poll_test_start_timer(state, Some(poll_stop_settle_cb), 100);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_does_not_flood_callbacks(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_no_flood_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn multi_poll_first_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<MultiPollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).first_callback_count += 1;
    (*state).passed &=
      (*state).first_callback_count == 1 && status == 0 && events == readable;
    if (*state).first_ready.send(()).is_err() {
      multi_poll_test_finish(state, false);
      return;
    }

    // The helper writes only after this callback begins and acknowledges the
    // write before it returns, proving the second descriptor is ready here.
    // The second callback then verifies it dispatches after this flag is set.
    if !matches!((*state).second_write_result.recv(), Ok(1)) {
      multi_poll_test_finish(state, false);
      return;
    }
    assert_eq!(libuv_sys_lite::uv_poll_stop(poll), 0);
    (*state).first_callback_returned = true;
  }
}

#[cfg(unix)]
unsafe extern "C" fn multi_poll_second_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<MultiPollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).second_callback_count += 1;
    (*state).passed &= (*state).first_callback_returned
      && (*state).second_callback_count == 1
      && status == 0
      && events == readable;
    multi_poll_test_finish(state, (*state).passed);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_delivers_two_fds(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_multi_poll_test(env, poll_callback_arg(env, info));
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    assert_eq!(
      libuv_sys_lite::uv_poll_start(
        (*state).first_poll,
        readable,
        Some(multi_poll_first_cb),
      ),
      0
    );
    assert_eq!(
      libuv_sys_lite::uv_poll_start(
        (*state).second_poll,
        readable,
        Some(multi_poll_second_cb),
      ),
      0
    );
    assert_eq!(
      libc::write((*state).first_writer.as_raw_fd(), b"x".as_ptr().cast(), 1,),
      1
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_self_stop_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).poll_callback_count += 1;
    (*state).passed &=
      (*state).poll_callback_count == 1 && status == 0 && events == readable;
    assert_eq!(libuv_sys_lite::uv_poll_stop(poll), 0);
    poll_test_finish(state, (*state).passed);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_self_stops(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_self_stop_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_self_restarted_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let writable = libuv_sys_lite::uv_poll_event::UV_WRITABLE.0 as i32;
    (*state).poll_callback_count += 1;
    // Two writable callbacks prove the replacement watch remains active after
    // its first delivery.
    (*state).passed &= (*state).poll_callback_count > 1
      && (*state).poll_callback_count <= 3
      && status == 0
      && events == writable;
    if (*state).poll_callback_count == 3 {
      assert_eq!(libuv_sys_lite::uv_poll_stop(poll), 0);
      poll_test_finish(state, (*state).passed);
    }
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_self_restart_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    let writable = libuv_sys_lite::uv_poll_event::UV_WRITABLE.0 as i32;
    (*state).poll_callback_count += 1;
    (*state).passed &=
      (*state).poll_callback_count == 1 && status == 0 && events == readable;
    // Keep the original byte unread while requesting writable so the
    // replacement runs while the original readiness remains.
    assert_eq!(
      libuv_sys_lite::uv_poll_start(
        poll,
        writable,
        Some(poll_self_restarted_cb)
      ),
      0
    );
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_restarts_in_callback(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let (client, server) = std::os::unix::net::UnixStream::pair().unwrap();
    let state = new_poll_test_with_fds(
      env,
      poll_callback_arg(env, info),
      client.into(),
      server.into(),
    );
    poll_test_write(state);
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_self_restart_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_self_close_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;
    (*state).poll_callback_count += 1;
    (*state).passed &=
      (*state).poll_callback_count == 1 && status == 0 && events == readable;
    (*state).poll_closed = true;
    libuv_sys_lite::uv_close(poll.cast(), Some(poll_test_close_cb));
    poll_test_finish(state, (*state).passed);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_closes_in_callback(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_self_close_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_restarted_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  status: i32,
  events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    let writable = libuv_sys_lite::uv_poll_event::UV_WRITABLE.0 as i32;
    (*state).passed &= status == 0 && events == writable;
    (*state).poll_callback_count += 1;
    // Requiring two callbacks also proves the replacement watch remains armed.
    if (*state).poll_callback_count == 2 {
      poll_test_finish(state, (*state).passed);
    }
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_stale_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  _status: i32,
  _events: i32,
) {
  unsafe {
    poll_test_finish((*poll).data.cast(), false);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_restart_replaces_watch(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let (client, server) = std::os::unix::net::UnixStream::pair().unwrap();
    let state = new_poll_test_with_fds(
      env,
      poll_callback_arg(env, info),
      client.into(),
      server.into(),
    );
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_stale_cb),
    );
    // Restarting replaces the callback and event mask atomically from the
    // addon's perspective; callbacks belonging to the previous watch must not
    // run. The socket is writable, while no data makes it readable.
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_WRITABLE.0 as i32,
      Some(poll_restarted_cb),
    );
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn duplicate_poll_close_cb(
  handle: *mut libuv_sys_lite::uv_handle_t,
) {
  unsafe {
    let _ = Box::from_raw(handle.cast::<libuv_sys_lite::uv_poll_t>());
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_allows_one_active_handle_per_fd(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    let second = Box::into_raw(Box::new(MaybeUninit::<
      libuv_sys_lite::uv_poll_t,
    >::zeroed()))
    .cast();
    let mut loop_ = null_mut();
    assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));
    let fd = (*state).poll_fd.as_raw_fd();
    let readable = libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32;

    // libuv permits multiple initialized poll handles for an fd, but only one
    // may actively poll it at a time. Exclusivity ends immediately when the
    // active handle stops.
    let second_init = libuv_sys_lite::uv_poll_init(loop_.cast(), second, fd);
    if second_init != 0 {
      // A failed init leaves the handle uninitialized, so uv_close() would not
      // be valid. Free its storage and report the contract failure directly.
      let _ = Box::from_raw(second);
      poll_test_finish(state, false);
      return null_mut();
    }
    libuv_sys_lite::uv_handle_set_data(second.cast(), state.cast());
    let start_zero =
      libuv_sys_lite::uv_poll_start((*state).poll, 0, Some(poll_stale_cb));
    let start_zero_null = libuv_sys_lite::uv_poll_start((*state).poll, 0, None);
    let first_start = libuv_sys_lite::uv_poll_start(
      (*state).poll,
      readable,
      Some(poll_stale_cb),
    );
    let first_start_null =
      libuv_sys_lite::uv_poll_start((*state).poll, readable, None);
    let third = Box::into_raw(Box::new(
      MaybeUninit::<libuv_sys_lite::uv_poll_t>::zeroed(),
    ))
    .cast();
    let late_init = libuv_sys_lite::uv_poll_init(loop_.cast(), third, fd);
    let conflicting_start =
      libuv_sys_lite::uv_poll_start(second, readable, Some(poll_stale_cb));
    let conflicting_start_zero =
      libuv_sys_lite::uv_poll_start(second, 0, Some(poll_stale_cb));
    let conflicting_start_zero_null =
      libuv_sys_lite::uv_poll_start(second, 0, None);
    let conflicting_start_null =
      libuv_sys_lite::uv_poll_start(second, readable, None);
    let first_stop = libuv_sys_lite::uv_poll_stop((*state).poll);
    let second_start =
      libuv_sys_lite::uv_poll_start(second, readable, Some(poll_stale_cb));
    let second_stop = libuv_sys_lite::uv_poll_stop(second);

    let passed = second_init == 0
      && start_zero == 0
      && start_zero_null == 0
      && first_start == 0
      && first_start_null == libuv_sys_lite::uv_errno_t::UV_EINVAL.0
      && late_init == libuv_sys_lite::uv_errno_t::UV_EEXIST.0
      && conflicting_start == libuv_sys_lite::uv_errno_t::UV_EEXIST.0
      && conflicting_start_zero == libuv_sys_lite::uv_errno_t::UV_EEXIST.0
      && conflicting_start_zero_null == libuv_sys_lite::uv_errno_t::UV_EEXIST.0
      && conflicting_start_null == libuv_sys_lite::uv_errno_t::UV_EEXIST.0
      && first_stop == 0
      && second_start == 0
      && second_stop == 0;

    libuv_sys_lite::uv_close(second.cast(), Some(duplicate_poll_close_cb));
    if late_init == 0 {
      libuv_sys_lite::uv_close(third.cast(), Some(duplicate_poll_close_cb));
    } else {
      let _ = Box::from_raw(third);
    }
    poll_test_finish(state, passed);
  }
  null_mut()
}

#[cfg(unix)]
unsafe extern "C" fn poll_after_close_cb(
  poll: *mut libuv_sys_lite::uv_poll_t,
  _status: i32,
  _events: i32,
) {
  unsafe {
    let state = (*poll).data.cast::<PollTest>();
    (*state).passed = false;
  }
}

#[cfg(unix)]
unsafe extern "C" fn poll_arm_then_close_cb(
  timer: *mut libuv_sys_lite::uv_timer_t,
) {
  unsafe {
    let state = (*timer).data.cast::<PollTest>();
    poll_test_start(
      state,
      libuv_sys_lite::uv_poll_event::UV_READABLE.0 as i32,
      Some(poll_after_close_cb),
    );
    // As in the stop test, use a generous scheduling window because the task
    // queue exposes no test-visible latch. uv_close() must invalidate the
    // callback expected to be in transit.
    std::thread::sleep(Duration::from_millis(100));
    (*state).poll_closed = true;
    libuv_sys_lite::uv_close((*state).poll.cast(), Some(poll_test_close_cb));
    poll_test_start_timer(state, Some(poll_stop_settle_cb), 100);
  }
}

#[cfg(unix)]
extern "C" fn test_uv_poll_close_suppresses_ready_callback(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  unsafe {
    let state = new_poll_test(env, poll_callback_arg(env, info));
    poll_test_write(state);
    assert_eq!(libuv_sys_lite::uv_timer_stop((*state).timer), 0);
    poll_test_start_timer(state, Some(poll_arm_then_close_cb), 0);
  }
  null_mut()
}

#[cfg(not(unix))]
macro_rules! unsupported_poll_test {
  ($name:ident) => {
    extern "C" fn $name(
      env: napi_env,
      _info: napi_callback_info,
    ) -> napi_value {
      let mut undefined = null_mut();
      unsafe {
        assert_napi_ok!(napi_get_undefined(env, &mut undefined));
      }
      undefined
    }
  };
}

#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_init_sets_nonblocking);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_reports_actual_writable_events);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_dispatches_hangup_only);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_invalid_fd_reports_ebadf);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_repeats_while_readable);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_stop_suppresses_ready_callback);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_does_not_flood_callbacks);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_restart_replaces_watch);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_allows_one_active_handle_per_fd);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_close_suppresses_ready_callback);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_delivers_two_fds);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_self_stops);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_restarts_in_callback);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_closes_in_callback);
#[cfg(not(unix))]
unsupported_poll_test!(test_uv_poll_leave_active);

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_uv_async", test_uv_async),
    napi_new_property!(env, "test_uv_async_ref", test_uv_async_ref),
    napi_new_property!(
      env,
      "test_uv_async_close_after_send",
      test_uv_async_close_after_send
    ),
    napi_new_property!(env, "test_uv_polyfills", test_uv_polyfills),
    napi_new_property!(env, "test_uv_timer_fires", test_uv_timer_fires),
    napi_new_property!(env, "test_uv_loop_helpers", test_uv_loop_helpers),
    napi_new_property!(env, "test_uv_threads", test_uv_threads),
    napi_new_property!(env, "test_uv_cond", test_uv_cond),
    napi_new_property!(env, "test_uv_cond_broadcast", test_uv_cond_broadcast),
    napi_new_property!(
      env,
      "test_uv_poll_init_sets_nonblocking",
      test_uv_poll_init_sets_nonblocking
    ),
    napi_new_property!(
      env,
      "test_uv_poll_reports_actual_writable_events",
      test_uv_poll_reports_actual_writable_events
    ),
    napi_new_property!(
      env,
      "test_uv_poll_dispatches_hangup_only",
      test_uv_poll_dispatches_hangup_only
    ),
    napi_new_property!(
      env,
      "test_uv_poll_reports_disconnect",
      test_uv_poll_reports_disconnect
    ),
    napi_new_property!(
      env,
      "test_uv_poll_invalid_fd_reports_ebadf",
      test_uv_poll_invalid_fd_reports_ebadf
    ),
    napi_new_property!(
      env,
      "test_uv_poll_repeats_while_readable",
      test_uv_poll_repeats_while_readable
    ),
    napi_new_property!(
      env,
      "test_uv_poll_stop_suppresses_ready_callback",
      test_uv_poll_stop_suppresses_ready_callback
    ),
    napi_new_property!(
      env,
      "test_uv_poll_does_not_flood_callbacks",
      test_uv_poll_does_not_flood_callbacks
    ),
    napi_new_property!(
      env,
      "test_uv_poll_restart_replaces_watch",
      test_uv_poll_restart_replaces_watch
    ),
    napi_new_property!(
      env,
      "test_uv_poll_allows_one_active_handle_per_fd",
      test_uv_poll_allows_one_active_handle_per_fd
    ),
    napi_new_property!(
      env,
      "test_uv_poll_close_suppresses_ready_callback",
      test_uv_poll_close_suppresses_ready_callback
    ),
    napi_new_property!(
      env,
      "test_uv_poll_delivers_two_fds",
      test_uv_poll_delivers_two_fds
    ),
    napi_new_property!(env, "test_uv_poll_self_stops", test_uv_poll_self_stops),
    napi_new_property!(
      env,
      "test_uv_poll_leave_active",
      test_uv_poll_leave_active
    ),
    napi_new_property!(
      env,
      "test_uv_poll_restarts_in_callback",
      test_uv_poll_restarts_in_callback
    ),
    napi_new_property!(
      env,
      "test_uv_poll_closes_in_callback",
      test_uv_poll_closes_in_callback
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
