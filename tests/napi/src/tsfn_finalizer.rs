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
use std::sync::atomic::AtomicU32;
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

// Progress counters, read back from JS via `get_tsfn_wrap_finalizer_stats` so
// the test can assert that the scenario actually ran. Without that check a
// silently degraded repro (e.g. the threadsafe function never dispatching)
// would pass vacuously.
static WRAPPED: AtomicU32 = AtomicU32::new(0);
static FINALIZED_COUNT: AtomicU32 = AtomicU32::new(0);

/// `napi_threadsafe_function` is documented as safe to use from any thread;
/// the raw pointer just isn't `Send`, so hand it to the worker thread wrapped.
struct SendTsfn(napi_threadsafe_function);

// SAFETY: see above — the NAPI contract permits cross-thread use of a tsfn.
unsafe impl Send for SendTsfn {}

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
  FINALIZED_COUNT.fetch_add(1, Ordering::Relaxed);
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
    WRAPPED.fetch_add(1, Ordering::Relaxed);
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
  // Deliver callbacks for a while from another thread; the stream outlives the
  // synchronous test body, so the runner's teardown/settling dispatches some
  // too.
  //
  // The tsfn is created with an initial thread count of 1, held by this
  // thread. Rather than acquiring a second count, that single count is handed
  // off to the worker thread, which releases it when it is done — so the tsfn
  // is closed exactly once and never outlives the worker.
  let tsfn = SendTsfn(tsfn);
  thread::spawn(move || {
    // Bind the whole `SendTsfn` (which is `Send`) before projecting to its
    // field, so RFC 2229 disjoint closure capture takes the wrapper rather than
    // the inner non-`Send` pointer.
    let tsfn = tsfn;
    let SendTsfn(tsfn) = tsfn;
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

// Returns `{ wrapped, finalized }` so the JS side can wait for the threadsafe
// function to actually start dispatching before it finishes.
extern "C" fn get_tsfn_wrap_finalizer_stats(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut result));

  for (key, value) in [
    ("wrapped\0", WRAPPED.load(Ordering::Relaxed)),
    ("finalized\0", FINALIZED_COUNT.load(Ordering::Relaxed)),
  ] {
    let mut num = ptr::null_mut();
    assert_napi_ok!(napi_create_uint32(env, value, &mut num));
    assert_napi_ok!(napi_set_named_property(
      env,
      result,
      key.as_ptr() as *const c_char,
      num
    ));
  }

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::napi_new_property!(
      env,
      "test_tsfn_wrap_finalizer",
      test_tsfn_wrap_finalizer
    ),
    crate::napi_new_property!(
      env,
      "get_tsfn_wrap_finalizer_stats",
      get_tsfn_wrap_finalizer_stats
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
