// Copyright 2018-2026 the Deno authors. MIT license.

use std::os::raw::c_char;
use std::os::raw::c_void;
use std::ptr;

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_function;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

pub struct Baton {
  called: bool,
  func: napi_ref,
  task: napi_async_work,
}

unsafe extern "C" fn execute(_env: napi_env, data: *mut c_void) {
  unsafe {
    let baton: &mut Baton = &mut *(data as *mut Baton);
    assert!(!baton.called);
    assert!(!baton.func.is_null());

    baton.called = true;
  }
}

unsafe extern "C" fn complete(
  env: napi_env,
  status: napi_status,
  data: *mut c_void,
) {
  unsafe {
    assert!(status == napi_ok);
    let baton: Box<Baton> = Box::from_raw(data as *mut Baton);
    assert!(baton.called);
    assert!(!baton.func.is_null());

    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));

    let mut callback: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_reference_value(env, baton.func, &mut callback));

    let mut _result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      callback,
      0,
      ptr::null(),
      &mut _result
    ));
    assert_napi_ok!(napi_delete_reference(env, baton.func));
    assert_napi_ok!(napi_delete_async_work(env, baton.task));
  }
}

extern "C" fn test_async_work(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_function);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    "test_async_resource".as_ptr() as *const c_char,
    usize::MAX,
    &mut resource_name,
  ));

  let async_work: napi_async_work = ptr::null_mut();

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));
  let baton = Box::new(Baton {
    called: false,
    func,
    task: async_work,
  });
  let mut async_work = baton.task;
  let baton_ptr = Box::into_raw(baton) as *mut c_void;

  assert_napi_ok!(napi_create_async_work(
    env,
    ptr::null_mut(),
    resource_name,
    Some(execute),
    Some(complete),
    baton_ptr,
    &mut async_work,
  ));
  let mut baton = unsafe { Box::from_raw(baton_ptr as *mut Baton) };
  baton.task = async_work;
  let _ = Box::into_raw(baton);
  assert_napi_ok!(napi_queue_async_work(env, async_work));

  ptr::null_mut()
}

// Test that async work's execute callback runs on a worker thread by calling
// a threadsafe function from it. This would deadlock if execute ran on the
// main thread (the pattern that lmdb-js uses).

struct TsfnBaton {
  tsfn: napi_threadsafe_function,
  task: napi_async_work,
  func: napi_ref,
}

unsafe extern "C" fn tsfn_execute(_env: napi_env, data: *mut c_void) {
  unsafe {
    let baton: &TsfnBaton = &*(data as *const TsfnBaton);
    // Call the threadsafe function from the execute (worker) thread.
    // This would deadlock if execute ran on the main thread.
    assert_eq!(
      napi_call_threadsafe_function(
        baton.tsfn,
        ptr::null_mut(),
        ThreadsafeFunctionCallMode::blocking,
      ),
      napi_ok
    );
  }
}

unsafe extern "C" fn tsfn_call_js(
  env: napi_env,
  _js_callback: napi_value,
  _context: *mut c_void,
  _data: *mut c_void,
) {
  // Release the tsfn from the JS thread after being called.
  // We get the tsfn from the callback info that was stashed earlier.
  // For simplicity, just do nothing here — the complete callback handles cleanup.
  let _ = env;
}

unsafe extern "C" fn tsfn_complete(
  env: napi_env,
  status: napi_status,
  data: *mut c_void,
) {
  unsafe {
    assert!(status == napi_ok);
    let baton: Box<TsfnBaton> = Box::from_raw(data as *mut TsfnBaton);

    // Release the threadsafe function
    assert_eq!(
      napi_release_threadsafe_function(
        baton.tsfn,
        ThreadsafeFunctionReleaseMode::release,
      ),
      napi_ok
    );

    // Call the JS callback to signal completion
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut callback: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_reference_value(env, baton.func, &mut callback));
    let mut _result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      callback,
      0,
      ptr::null(),
      &mut _result
    ));
    assert_napi_ok!(napi_delete_reference(env, baton.func));
    assert_napi_ok!(napi_delete_async_work(env, baton.task));
  }
}

extern "C" fn test_async_work_with_tsfn(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    "test_async_tsfn".as_ptr() as *const c_char,
    usize::MAX,
    &mut resource_name,
  ));

  // Create a threadsafe function
  let mut tsfn: napi_threadsafe_function = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    ptr::null_mut(),    // func (unused, we use call_js_cb)
    ptr::null_mut(),    // async_resource
    resource_name,      // async_resource_name
    0,                  // max_queue_size (unlimited)
    1,                  // initial_thread_count
    ptr::null_mut(),    // thread_finalize_data
    None,               // thread_finalize_cb
    ptr::null_mut(),    // context
    Some(tsfn_call_js), // call_js_cb
    &mut tsfn,
  ));

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));

  let baton = Box::new(TsfnBaton {
    tsfn,
    task: ptr::null_mut(),
    func,
  });
  let baton_ptr = Box::into_raw(baton) as *mut c_void;

  let mut async_work: napi_async_work = ptr::null_mut();
  assert_napi_ok!(napi_create_async_work(
    env,
    ptr::null_mut(),
    resource_name,
    Some(tsfn_execute),
    Some(tsfn_complete),
    baton_ptr,
    &mut async_work,
  ));
  let baton = unsafe { &mut *(baton_ptr as *mut TsfnBaton) };
  baton.task = async_work;
  assert_napi_ok!(napi_queue_async_work(env, async_work));

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_async_work", test_async_work),
    napi_new_property!(
      env,
      "test_async_work_with_tsfn",
      test_async_work_with_tsfn
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
