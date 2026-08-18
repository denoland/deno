// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use napi_sys::ValueType::napi_object;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

unsafe extern "C" fn finalize_cb(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  assert!(data.is_null());
  assert!(hint.is_null());
}

extern "C" fn test_bind_finalizer(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_object);

  let obj = args[0];
  unsafe {
    napi_add_finalizer(
      env,
      obj,
      ptr::null_mut(),
      Some(finalize_cb),
      ptr::null_mut(),
      ptr::null_mut(),
    )
  };
  obj
}

struct Thing {
  _allocation: Vec<u8>,
}

unsafe extern "C" fn finalize_cb_drop(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  unsafe {
    let _ = Box::from_raw(data as *mut Thing);
    assert!(hint.is_null());
  }
}

extern "C" fn test_external_finalizer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let data = Box::into_raw(Box::new(Thing {
    _allocation: vec![1, 2, 3],
  }));

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    data as _,
    Some(finalize_cb_drop),
    ptr::null_mut(),
    &mut result
  ));
  result
}

unsafe extern "C" fn finalize_cb_vec(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  unsafe {
    let _ = Vec::from_raw_parts(data as *mut u8, 3, 3);
    assert!(hint.is_null());
  }
}

extern "C" fn test_external_buffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let buf: Vec<u8> = vec![1, 2, 3];
  assert_napi_ok!(napi_create_external_buffer(
    env,
    3,
    buf.as_ptr() as _,
    Some(finalize_cb_vec),
    ptr::null_mut(),
    &mut result
  ));
  std::mem::forget(buf);

  result
}

extern "C" fn test_static_external_buffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  static BUF: &[u8] = &[1, 2, 3];
  assert_napi_ok!(napi_create_external_buffer(
    env,
    BUF.len(),
    BUF.as_ptr() as _,
    None,
    ptr::null_mut(),
    &mut result
  ));

  result
}

extern "C" fn test_external_arraybuffer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let buf: Vec<u8> = vec![1, 2, 3];
  assert_napi_ok!(napi_create_external_arraybuffer(
    env,
    buf.as_ptr() as _,
    3,
    Some(finalize_cb_vec),
    ptr::null_mut(),
    &mut result
  ));
  std::mem::forget(buf);

  result
}

/// Wrap finalizer that prints a message. Used to test that wrap finalizers
/// are called at shutdown even when the wrapped object is still reachable.
unsafe extern "C" fn wrap_leak_release(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  _hint: *mut ::std::os::raw::c_void,
) {
  let msg = unsafe { Box::from_raw(data as *mut String) };
  println!("{}", msg);
}

/// Creates a napi_wrap on the given JS object with a finalizer that prints
/// a message. The wrap is NOT explicitly removed, so only the shutdown
/// finalizer path will trigger the callback. This tests that NAPI wrap
/// finalizers are called during environment teardown (matching Node.js
/// behavior).
extern "C" fn test_wrap_leak(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_object);

  let msg = Box::new(String::from("pointers released on shutdown"));
  let msg_ptr = Box::into_raw(msg) as *mut ::std::os::raw::c_void;

  assert_napi_ok!(napi_wrap(
    env,
    args[0],
    msg_ptr,
    Some(wrap_leak_release),
    ptr::null_mut(),
    ptr::null_mut(),
  ));

  args[0]
}

/// Flag for testing deferred finalizer behavior.
static DEFERRED_FINALIZER_RAN: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn deferred_finalize_cb(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  _hint: *mut ::std::os::raw::c_void,
) {
  unsafe {
    let _ = Box::from_raw(data as *mut Thing);
  }
  DEFERRED_FINALIZER_RAN.store(true, Ordering::SeqCst);
}

/// Creates an external value with a finalizer that sets a flag when called.
/// Used to test that GC finalizers are deferred to the event loop.
extern "C" fn test_deferred_finalizer(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  // Reset the flag
  DEFERRED_FINALIZER_RAN.store(false, Ordering::SeqCst);

  let data = Box::into_raw(Box::new(Thing {
    _allocation: vec![1, 2, 3],
  }));

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    data as _,
    Some(deferred_finalize_cb),
    ptr::null_mut(),
    &mut result
  ));
  result
}

/// Returns whether the deferred finalizer has been called.
extern "C" fn test_deferred_finalizer_check(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let ran = DEFERRED_FINALIZER_RAN.load(Ordering::SeqCst);
  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, ran, &mut result));
  result
}

/// Ids of the externals created by `test_shared_data_external` that have not
/// been finalized yet.
static SHARED_DATA_LIVE: std::sync::Mutex<
  Option<std::collections::HashSet<usize>>,
> = std::sync::Mutex::new(None);
static SHARED_DATA_FINALIZED: std::sync::atomic::AtomicUsize =
  std::sync::atomic::AtomicUsize::new(0);
static SHARED_DATA_DOUBLE: std::sync::atomic::AtomicUsize =
  std::sync::atomic::AtomicUsize::new(0);

/// All externals created by `test_shared_data_external` share this `data`
/// pointer while each has its own `hint`, the shape addons like
/// `@duckdb/node-api` produce through `Napi::External`. The finalizer must be
/// called exactly once per external.
unsafe extern "C" fn shared_data_finalize_cb(
  _env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  assert!(data.is_null());
  let id = hint as usize;
  let mut live = SHARED_DATA_LIVE.lock().unwrap();
  let live = live.get_or_insert_with(Default::default);
  if !live.remove(&id) {
    SHARED_DATA_DOUBLE.fetch_add(1, Ordering::SeqCst);
    panic!("finalizer for external {id} called more than once");
  }
  SHARED_DATA_FINALIZED.fetch_add(1, Ordering::SeqCst);
}

extern "C" fn test_shared_data_external(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut id: u32 = 0;
  assert_napi_ok!(napi_get_value_uint32(env, args[0], &mut id));
  // Ids start at 1 so that no id collides with a null `hint`.
  let id = id as usize + 1;

  {
    let mut live = SHARED_DATA_LIVE.lock().unwrap();
    assert!(live.get_or_insert_with(Default::default).insert(id));
  }

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    ptr::null_mut(),
    Some(shared_data_finalize_cb),
    id as *mut ::std::os::raw::c_void,
    &mut result
  ));
  result
}

extern "C" fn test_shared_data_finalized_count(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let count = SHARED_DATA_FINALIZED.load(Ordering::SeqCst);
  assert_napi_ok!(napi_create_uint32(env, count as u32, &mut result));
  result
}

extern "C" fn test_shared_data_double_count(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  let count = SHARED_DATA_DOUBLE.load(Ordering::SeqCst);
  assert_napi_ok!(napi_create_uint32(env, count as u32, &mut result));
  result
}

/// Finalizer that calls back into JavaScript, the shape of `test_finalizer`
/// and `test_function` in the Node-API conformance suite. It must run at a
/// point where JS execution is legal; running it in V8's GC weak callback
/// aborts the process. See #36568.
unsafe extern "C" fn finalize_calls_js(
  env: napi_env,
  _data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  let cb_ref = hint as napi_ref;
  let mut cb = ptr::null_mut();
  assert_napi_ok!(napi_get_reference_value(env, cb_ref, &mut cb));
  let mut global = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));
  // Release the reference before calling into JS: `cb` is a handle in the
  // current scope and stays valid, and the callback is allowed to throw, after
  // which every napi call on this env returns napi_pending_exception until the
  // runtime clears it.
  assert_napi_ok!(napi_delete_reference(env, cb_ref));
  let mut result = ptr::null_mut();
  // Ignore the status: the point is that calling into JS here does not abort.
  unsafe {
    napi_call_function(env, global, cb, 0, ptr::null(), &mut result);
  }
}

/// Creates an external whose finalizer invokes the JS callback passed as the
/// first argument.
extern "C" fn test_external_finalizer_calls_js(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut cb_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut cb_ref));

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    ptr::null_mut(),
    Some(finalize_calls_js),
    cb_ref as *mut ::std::os::raw::c_void,
    &mut result
  ));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(
      env,
      "test_external_finalizer_calls_js",
      test_external_finalizer_calls_js
    ),
    napi_new_property!(env, "test_bind_finalizer", test_bind_finalizer),
    napi_new_property!(env, "test_external_finalizer", test_external_finalizer),
    napi_new_property!(env, "test_external_buffer", test_external_buffer),
    napi_new_property!(
      env,
      "test_external_arraybuffer",
      test_external_arraybuffer
    ),
    napi_new_property!(
      env,
      "test_static_external_buffer",
      test_static_external_buffer
    ),
    napi_new_property!(env, "test_wrap_leak", test_wrap_leak),
    napi_new_property!(env, "test_deferred_finalizer", test_deferred_finalizer),
    napi_new_property!(
      env,
      "test_deferred_finalizer_check",
      test_deferred_finalizer_check
    ),
    napi_new_property!(
      env,
      "test_shared_data_external",
      test_shared_data_external
    ),
    napi_new_property!(
      env,
      "test_shared_data_finalized_count",
      test_shared_data_finalized_count
    ),
    napi_new_property!(
      env,
      "test_shared_data_double_count",
      test_shared_data_double_count
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
