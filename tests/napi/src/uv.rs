// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::addr_of_mut;
use std::ptr::null_mut;
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
  use std::ptr;
  use std::ptr::addr_of_mut;

  use libuv_sys_lite::uv_close;
  use libuv_sys_lite::uv_cpu_info;
  use libuv_sys_lite::uv_cpu_info_t;
  use libuv_sys_lite::uv_default_loop;
  use libuv_sys_lite::uv_free_cpu_info;
  use libuv_sys_lite::uv_handle_get_data;
  use libuv_sys_lite::uv_handle_set_data;
  use libuv_sys_lite::uv_handle_t;
  use libuv_sys_lite::uv_hrtime;
  use libuv_sys_lite::uv_is_active;
  use libuv_sys_lite::uv_is_closing;
  use libuv_sys_lite::uv_ref;
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
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
