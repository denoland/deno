// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/36499.
//
// A `napi_threadsafe_function` callback repeatedly wraps freshly created
// objects with a `napi_wrap` finalizer, then churns allocations so the GC
// runs while those wraps are still pending. Under `deno test`, the runner
// drains the NAPI finalizer queue at worker teardown; a wrap finalized there
// but garbage collected afterwards must NOT have its finalizer invoked twice.
// The finalizer below aborts on a double invocation so a regression fails the
// napi test process loudly instead of silently double-freeing.

use std::ffi::c_char;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use napi_sys::Status::napi_ok;
use napi_sys::*;

use crate::assert_napi_ok;

const LIVE: u32 = 0xA5A5_A5A5;
const FINALIZED: u32 = 0x5A5A_5A5A;

const BATCH: usize = 50;
const CHURN: usize = 250;
const TICKS: usize = 150;
const TICK: Duration = Duration::from_micros(200);

static TSFN: AtomicPtr<napi_threadsafe_function__> =
  AtomicPtr::new(ptr::null_mut());

// `napi_wrap` finalizer: detects a duplicate invocation for the same wrap.
// `data` is intentionally leaked so a second call reads live (poisoned) state
// instead of freed memory, turning a would-be double-free into a clean abort.
unsafe extern "C" fn finalize_cb(
  _env: napi_env,
  data: *mut c_void,
  _hint: *mut c_void,
) {
  let state = data as *mut u32;
  unsafe {
    if *state != LIVE {
      eprintln!(
        "FATAL: napi_wrap finalizer invoked twice for {:p} (#36499)",
        data
      );
      std::process::abort();
    }
    *state = FINALIZED;
  }
}

unsafe extern "C" fn noop(
  _env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  ptr::null_mut()
}

// Threadsafe-function callback: runs on the main thread with a live env.
extern "C" fn call_js_cb(
  env: napi_env,
  _js_cb: napi_value,
  _context: *mut c_void,
  _data: *mut c_void,
) {
  if env.is_null() {
    return;
  }
  let mut scope = ptr::null_mut();
  assert_napi_ok!(napi_open_handle_scope(env, &mut scope));
  // A batch of wrapped objects, garbage as soon as the scope closes.
  for _ in 0..BATCH {
    let mut obj = ptr::null_mut();
    assert_napi_ok!(napi_create_object(env, &mut obj));
    let state = Box::into_raw(Box::new(LIVE)) as *mut c_void;
    assert_napi_ok!(napi_wrap(
      env,
      obj,
      state,
      Some(finalize_cb),
      ptr::null_mut(),
      ptr::null_mut(),
    ));
  }
  // Enough allocation that GCs regularly land inside these callbacks.
  for _ in 0..CHURN {
    let mut obj = ptr::null_mut();
    assert_napi_ok!(napi_create_object(env, &mut obj));
  }
  assert_napi_ok!(napi_close_handle_scope(env, scope));
}

extern "C" fn test_tsfn_wrap_finalizer(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut js_cb = ptr::null_mut();
  assert_napi_ok!(napi_create_function(
    env,
    "noop\0".as_ptr() as *const c_char,
    4,
    Some(noop),
    ptr::null_mut(),
    &mut js_cb,
  ));
  let mut name = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    "repro\0".as_ptr() as *const c_char,
    5,
    &mut name,
  ));
  let mut tsfn = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    js_cb,
    ptr::null_mut(),
    name,
    0,
    1,
    ptr::null_mut(),
    None,
    ptr::null_mut(),
    Some(call_js_cb),
    &mut tsfn,
  ));
  TSFN.store(tsfn, Ordering::SeqCst);

  // Deliver callbacks for a while from another thread; the stream outlives the
  // synchronous test body, so the runner's teardown/settling dispatches some
  // too. `napi_threadsafe_function` is safe to call across threads by design.
  thread::spawn(move || {
    let tsfn = TSFN.load(Ordering::SeqCst);
    for _ in 0..TICKS {
      let status = unsafe {
        napi_call_threadsafe_function(
          tsfn,
          ptr::null_mut(),
          ThreadsafeFunctionCallMode::nonblocking,
        )
      };
      if status != napi_ok {
        break;
      }
      thread::sleep(TICK);
    }
    unsafe {
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::release,
      );
    }
  });

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[crate::napi_new_property!(
    env,
    "test_tsfn_wrap_finalizer",
    test_tsfn_wrap_finalizer
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
