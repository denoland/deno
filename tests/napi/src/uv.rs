// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::addr_of_mut;
use std::ptr::null_mut;
use std::time::Duration;

use libuv_sys_lite::uv_async_init;
use libuv_sys_lite::uv_async_t;
use libuv_sys_lite::uv_check_init;
use libuv_sys_lite::uv_check_start;
use libuv_sys_lite::uv_check_stop;
use libuv_sys_lite::uv_check_t;
use libuv_sys_lite::uv_close;
use libuv_sys_lite::uv_handle_t;
use libuv_sys_lite::uv_has_ref;
use libuv_sys_lite::uv_idle_init;
use libuv_sys_lite::uv_idle_start;
use libuv_sys_lite::uv_idle_stop;
use libuv_sys_lite::uv_idle_t;
use libuv_sys_lite::uv_is_active;
use libuv_sys_lite::uv_mutex_destroy;
use libuv_sys_lite::uv_mutex_lock;
use libuv_sys_lite::uv_mutex_t;
use libuv_sys_lite::uv_mutex_unlock;
use libuv_sys_lite::uv_os_getpid;
use libuv_sys_lite::uv_ref;
use libuv_sys_lite::uv_timer_get_repeat;
use libuv_sys_lite::uv_timer_init;
use libuv_sys_lite::uv_timer_set_repeat;
use libuv_sys_lite::uv_timer_start;
use libuv_sys_lite::uv_timer_stop;
use libuv_sys_lite::uv_timer_t;
use libuv_sys_lite::uv_unref;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn new_raw<T>(t: T) -> *mut T {
  Box::into_raw(Box::new(t))
}

// ---------------------------------------------------------------------------
// uv_async tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// uv_timer tests
// ---------------------------------------------------------------------------

struct TimerData {
  env: napi_env,
  callback: napi_ref,
  count: u32,
  _keep_alive: KeepAlive,
}

unsafe extern "C" fn timer_cb(handle: *mut uv_timer_t) {
  unsafe {
    let data = (*handle).data as *mut TimerData;
    (*data).count += 1;
    let env = (*data).env;

    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      env,
      (*data).callback,
      &mut js_cb
    ));
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut count_js = null_mut();
    assert_napi_ok!(napi_create_uint32(env, (*data).count, &mut count_js));
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      1,
      &count_js,
      &mut result,
    ));

    // After 3 firings, stop and close.
    if (*data).count >= 3 {
      uv_timer_stop(handle);
      uv_close(handle.cast(), Some(timer_close_cb));
    }
  }
}

unsafe extern "C" fn timer_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let timer = handle as *mut uv_timer_t;
    let data = Box::from_raw((*timer).data as *mut TimerData);
    let env = data.env;
    assert_napi_ok!(napi_delete_reference(env, data.callback));
    // The timer handle was heap-allocated by us; free it.
    let _ = Box::from_raw(timer);
  }
}

/// test_uv_timer(callback): starts a repeating timer at 10ms interval.
/// Calls `callback(count)` each time. After 3 firings, stops and closes.
#[allow(
  unused_unsafe,
  reason = "macro-generated code may wrap safe ops in unsafe"
)]
extern "C" fn test_uv_timer(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

  let timer = new_raw(MaybeUninit::<uv_timer_t>::uninit());
  let timer = timer.cast::<uv_timer_t>();

  unsafe {
    assert_napi_ok!(uv_timer_init(loop_.cast(), timer));
  }

  let mut js_cb = null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));

  let data = new_raw(TimerData {
    env,
    callback: js_cb,
    count: 0,
    _keep_alive: KeepAlive::new(env),
  });
  unsafe {
    addr_of_mut!((*timer).data).write(data.cast());
    // repeat every 10ms
    assert_napi_ok!(uv_timer_start(timer, Some(timer_cb), 10, 10));
  }

  ptr::null_mut()
}

/// test_uv_timer_ref_unref(callback): starts a timer, unrefs it (so it
/// won't keep the event loop alive on its own), then refs it again.
/// Reports ref state via callback.
#[allow(
  unused_unsafe,
  reason = "macro-generated code may wrap safe ops in unsafe"
)]
extern "C" fn test_uv_timer_ref_unref(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

  let timer = new_raw(MaybeUninit::<uv_timer_t>::uninit());
  let timer = timer.cast::<uv_timer_t>();
  unsafe {
    assert_napi_ok!(uv_timer_init(loop_.cast(), timer));

    // Initially ref'd
    let has_ref_initial = uv_has_ref(timer.cast());
    assert_eq!(has_ref_initial, 1);

    // Unref
    uv_unref(timer.cast());
    let has_ref_after_unref = uv_has_ref(timer.cast());
    assert_eq!(has_ref_after_unref, 0);

    // Re-ref
    uv_ref(timer.cast());
    let has_ref_after_ref = uv_has_ref(timer.cast());
    assert_eq!(has_ref_after_ref, 1);

    // Check is_active before starting
    let active_before = uv_is_active(timer.cast());
    assert_eq!(active_before, 0);

    // Start timer
    assert_napi_ok!(uv_timer_start(timer, Some(ref_unref_timer_cb), 5, 0));

    // Check is_active after starting
    let active_after = uv_is_active(timer.cast());
    assert_eq!(active_after, 1);
  }

  let mut js_cb = null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  let data = new_raw(RefUnrefTimerData {
    env,
    callback: js_cb,
    _keep_alive: KeepAlive::new(env),
  });
  unsafe {
    addr_of_mut!((*timer).data).write(data.cast());
  }

  ptr::null_mut()
}

struct RefUnrefTimerData {
  env: napi_env,
  callback: napi_ref,
  _keep_alive: KeepAlive,
}

unsafe extern "C" fn ref_unref_timer_cb(handle: *mut uv_timer_t) {
  unsafe {
    let data = (*handle).data as *mut RefUnrefTimerData;
    let env = (*data).env;
    let mut js_cb = null_mut();
    assert_napi_ok!(napi_get_reference_value(
      env,
      (*data).callback,
      &mut js_cb
    ));
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    // Pass "ok" string to signal success
    let mut ok_str = null_mut();
    assert_napi_ok!(napi_create_string_utf8(
      env,
      c"ok".as_ptr(),
      2,
      &mut ok_str
    ));
    let mut result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      js_cb,
      1,
      &ok_str,
      &mut result,
    ));

    assert_napi_ok!(napi_delete_reference(env, (*data).callback));
    let _ = Box::from_raw(data);
    uv_close(handle.cast(), Some(ref_unref_timer_close_cb));
  }
}

unsafe extern "C" fn ref_unref_timer_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let _ = Box::from_raw(handle as *mut uv_timer_t);
  }
}

/// test_uv_timer_repeat(): tests set_repeat/get_repeat and timer_stop.
#[allow(
  unused_unsafe,
  reason = "macro-generated code may wrap safe ops in unsafe"
)]
extern "C" fn test_uv_timer_repeat(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 0);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

  let timer = new_raw(MaybeUninit::<uv_timer_t>::uninit());
  let timer = timer.cast::<uv_timer_t>();
  unsafe {
    assert_napi_ok!(uv_timer_init(loop_.cast(), timer));

    uv_timer_set_repeat(timer, 42);
    let repeat = uv_timer_get_repeat(timer);
    assert_eq!(repeat, 42);

    uv_timer_set_repeat(timer, 100);
    let repeat = uv_timer_get_repeat(timer);
    assert_eq!(repeat, 100);

    // Clean up
    uv_close(timer.cast(), Some(repeat_timer_close_cb));
  }

  // Return true to signal success
  let mut result = null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

unsafe extern "C" fn repeat_timer_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let _ = Box::from_raw(handle as *mut uv_timer_t);
  }
}

// ---------------------------------------------------------------------------
// uv_idle tests
// ---------------------------------------------------------------------------

struct IdleData {
  env: napi_env,
  callback: napi_ref,
  count: u32,
  _keep_alive: KeepAlive,
}

unsafe extern "C" fn idle_cb(handle: *mut uv_idle_t) {
  unsafe {
    let data = (*handle).data as *mut IdleData;
    (*data).count += 1;

    // Stop after 3 firings and close
    if (*data).count >= 3 {
      uv_idle_stop(handle);

      let env = (*data).env;
      let mut js_cb = null_mut();
      assert_napi_ok!(napi_get_reference_value(
        env,
        (*data).callback,
        &mut js_cb
      ));
      let mut global: napi_value = ptr::null_mut();
      assert_napi_ok!(napi_get_global(env, &mut global));
      let mut count_js = null_mut();
      assert_napi_ok!(napi_create_uint32(env, (*data).count, &mut count_js));
      let mut result: napi_value = ptr::null_mut();
      assert_napi_ok!(napi_call_function(
        env,
        global,
        js_cb,
        1,
        &count_js,
        &mut result,
      ));

      uv_close(handle.cast(), Some(idle_close_cb));
    }
  }
}

unsafe extern "C" fn idle_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let idle = handle as *mut uv_idle_t;
    let data = Box::from_raw((*idle).data as *mut IdleData);
    let env = data.env;
    assert_napi_ok!(napi_delete_reference(env, data.callback));
    let _ = Box::from_raw(idle);
  }
}

/// test_uv_idle(callback): starts an idle handle. It fires on every event
/// loop iteration. After 3 firings, calls callback(3) and closes.
#[allow(
  unused_unsafe,
  reason = "macro-generated code may wrap safe ops in unsafe"
)]
extern "C" fn test_uv_idle(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

  let idle = new_raw(MaybeUninit::<uv_idle_t>::uninit());
  let idle = idle.cast::<uv_idle_t>();
  unsafe {
    assert_napi_ok!(uv_idle_init(loop_.cast(), idle));
  }

  let mut js_cb = null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  let data = new_raw(IdleData {
    env,
    callback: js_cb,
    count: 0,
    _keep_alive: KeepAlive::new(env),
  });
  unsafe {
    addr_of_mut!((*idle).data).write(data.cast());
    assert_napi_ok!(uv_idle_start(idle, Some(idle_cb)));
  }

  ptr::null_mut()
}

// ---------------------------------------------------------------------------
// uv_check tests
// ---------------------------------------------------------------------------

struct CheckData {
  env: napi_env,
  callback: napi_ref,
  count: u32,
  _keep_alive: KeepAlive,
}

unsafe extern "C" fn check_cb(handle: *mut uv_check_t) {
  unsafe {
    let data = (*handle).data as *mut CheckData;
    (*data).count += 1;

    if (*data).count >= 3 {
      uv_check_stop(handle);

      let env = (*data).env;
      let mut js_cb = null_mut();
      assert_napi_ok!(napi_get_reference_value(
        env,
        (*data).callback,
        &mut js_cb
      ));
      let mut global: napi_value = ptr::null_mut();
      assert_napi_ok!(napi_get_global(env, &mut global));
      let mut count_js = null_mut();
      assert_napi_ok!(napi_create_uint32(env, (*data).count, &mut count_js));
      let mut result: napi_value = ptr::null_mut();
      assert_napi_ok!(napi_call_function(
        env,
        global,
        js_cb,
        1,
        &count_js,
        &mut result,
      ));

      uv_close(handle.cast(), Some(check_close_cb));
    }
  }
}

unsafe extern "C" fn check_close_cb(handle: *mut uv_handle_t) {
  unsafe {
    let check = handle as *mut uv_check_t;
    let data = Box::from_raw((*check).data as *mut CheckData);
    let env = data.env;
    assert_napi_ok!(napi_delete_reference(env, data.callback));
    let _ = Box::from_raw(check);
  }
}

/// test_uv_check(callback): starts a check handle. It fires on every
/// event loop iteration (check phase). After 3 firings, calls callback(3)
/// and closes.
#[allow(
  unused_unsafe,
  reason = "macro-generated code may wrap safe ops in unsafe"
)]
extern "C" fn test_uv_check(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut loop_ = null_mut();
  assert_napi_ok!(napi_get_uv_event_loop(env, &mut loop_));

  let check = new_raw(MaybeUninit::<uv_check_t>::uninit());
  let check = check.cast::<uv_check_t>();
  unsafe {
    assert_napi_ok!(uv_check_init(loop_.cast(), check));
  }

  let mut js_cb = null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut js_cb));
  let data = new_raw(CheckData {
    env,
    callback: js_cb,
    count: 0,
    _keep_alive: KeepAlive::new(env),
  });
  unsafe {
    addr_of_mut!((*check).data).write(data.cast());
    assert_napi_ok!(uv_check_start(check, Some(check_cb)));
  }

  ptr::null_mut()
}

// ---------------------------------------------------------------------------
// uv_os_getpid test
// ---------------------------------------------------------------------------

extern "C" fn test_uv_os_getpid(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 0);

  let pid = unsafe { uv_os_getpid() };
  let mut result = null_mut();
  assert_napi_ok!(napi_create_int32(env, pid, &mut result));
  result
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_uv_async", test_uv_async),
    napi_new_property!(env, "test_uv_async_ref", test_uv_async_ref),
    napi_new_property!(env, "test_uv_timer", test_uv_timer),
    napi_new_property!(env, "test_uv_timer_ref_unref", test_uv_timer_ref_unref),
    napi_new_property!(env, "test_uv_timer_repeat", test_uv_timer_repeat),
    napi_new_property!(env, "test_uv_idle", test_uv_idle),
    napi_new_property!(env, "test_uv_check", test_uv_check),
    napi_new_property!(env, "test_uv_os_getpid", test_uv_os_getpid),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
