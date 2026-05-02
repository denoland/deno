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
  // For simplicity, just do nothing here. The complete callback handles cleanup.
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

// Test that call_js_cb receives a valid (non-null) env even when the tsfn
// is closed before all queued calls are processed. This reproduces a crash
// seen with node-pty: a race between calling and releasing threads causes
// TsFn::drop to run before a pending call, and the call_js_cb was invoked
// with env=NULL, crashing addons that dereference env without a null check.

unsafe extern "C" fn tsfn_race_call_js(
  env: napi_env,
  _js_callback: napi_value,
  _context: *mut c_void,
  _data: *mut c_void,
) {
  // Simulate what node-pty does: use the env parameter without null check.
  // Before the fix, env could be null here when the tsfn was already closed.
  assert!(
    !env.is_null(),
    "call_js_cb received null env, this is the bug that crashes node-pty"
  );
  unsafe {
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
  }
}

unsafe extern "C" fn tsfn_race_finalize(
  env: napi_env,
  finalize_data: *mut c_void,
  _hint: *mut c_void,
) {
  // Called when the tsfn is fully closed. Signal completion to JS.
  unsafe {
    let func = finalize_data as napi_ref;
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut callback: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_reference_value(env, func, &mut callback));
    let mut _result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      callback,
      0,
      ptr::null(),
      &mut _result
    ));
    assert_napi_ok!(napi_delete_reference(env, func));
  }
}

extern "C" fn test_tsfn_close_race(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"tsfn_race".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));

  let mut tsfn: napi_threadsafe_function = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    ptr::null_mut(), // no JS func
    ptr::null_mut(), // no async resource
    resource_name,
    0,                        // unlimited queue
    1,                        // initial_thread_count
    func as *mut c_void,      // thread_finalize_data
    Some(tsfn_race_finalize), // thread_finalize_cb
    ptr::null_mut(),          // context
    Some(tsfn_race_call_js),  // call_js_cb
    &mut tsfn,
  ));

  // napi_threadsafe_function is a raw pointer which is !Send.
  // Cast to usize so we can move it into std::thread::spawn.
  let tsfn_addr = tsfn as usize;

  // Thread A: calls the tsfn many times. The calls are queued to the main
  // thread via sender.spawn. Some of these calls may be processed after the
  // tsfn is dropped (if Thread B's release wins the spawn race).
  let tsfn_a = tsfn_addr;
  std::thread::spawn(move || {
    let tsfn = tsfn_a as napi_threadsafe_function;
    for _ in 0..100 {
      let status = unsafe {
        napi_call_threadsafe_function(
          tsfn,
          ptr::null_mut(),
          ThreadsafeFunctionCallMode::nonblocking,
        )
      };
      if status != napi_ok {
        break; // tsfn is closing
      }
    }
  });

  // Thread B: releases the tsfn, which triggers the close (thread_count
  // drops from 1 to 0). The drop is queued via sender.spawn. If Thread A's
  // sender.spawn calls are interleaved with this, some calls will land in
  // the queue after the drop. Those calls see is_closed=true.
  let tsfn_b = tsfn_addr;
  std::thread::spawn(move || {
    let tsfn = tsfn_b as napi_threadsafe_function;
    unsafe {
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::release,
      );
    }
  });

  ptr::null_mut()
}

// Same as test_tsfn_close_race but uses napi_tsfn_abort instead of
// napi_tsfn_release to close the tsfn. Abort mode immediately marks
// the tsfn as closing regardless of thread_count.
extern "C" fn test_tsfn_abort_race(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"tsfn_abort".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));

  let mut tsfn: napi_threadsafe_function = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    ptr::null_mut(),
    ptr::null_mut(),
    resource_name,
    0,
    1,
    func as *mut c_void,
    Some(tsfn_race_finalize),
    ptr::null_mut(),
    Some(tsfn_race_call_js),
    &mut tsfn,
  ));

  let tsfn_addr = tsfn as usize;

  let tsfn_a = tsfn_addr;
  std::thread::spawn(move || {
    let tsfn = tsfn_a as napi_threadsafe_function;
    for _ in 0..100 {
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
    }
  });

  // Use abort mode to close. This forces close regardless of thread_count.
  let tsfn_b = tsfn_addr;
  std::thread::spawn(move || {
    let tsfn = tsfn_b as napi_threadsafe_function;
    unsafe {
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::abort,
      );
    }
  });

  ptr::null_mut()
}

// Test napi_acquire_threadsafe_function and napi_release_threadsafe_function.
// Creates a tsfn with initial_thread_count=1, acquires it (count=2),
// releases twice (count drops to 1 then 0, triggering close).
// The finalize callback calls the JS callback to signal success.

unsafe extern "C" fn tsfn_acquire_release_finalize(
  env: napi_env,
  finalize_data: *mut c_void,
  _hint: *mut c_void,
) {
  unsafe {
    let func = finalize_data as napi_ref;
    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));
    let mut callback: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_reference_value(env, func, &mut callback));
    let mut _result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      callback,
      0,
      ptr::null(),
      &mut _result
    ));
    assert_napi_ok!(napi_delete_reference(env, func));
  }
}

extern "C" fn test_tsfn_acquire_release(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"tsfn_acquire_release".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));

  let mut tsfn: napi_threadsafe_function = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    ptr::null_mut(), // no JS func
    ptr::null_mut(), // no async resource
    resource_name,
    0,                                   // unlimited queue
    1,                                   // initial_thread_count
    func as *mut c_void,                 // thread_finalize_data
    Some(tsfn_acquire_release_finalize), // thread_finalize_cb
    ptr::null_mut(),                     // context
    Some(tsfn_race_call_js),             // call_js_cb (reuse existing no-op)
    &mut tsfn,
  ));

  // Acquire: thread_count goes from 1 to 2
  assert_napi_ok!(napi_acquire_threadsafe_function(tsfn));

  // Release twice from a thread: 2 -> 1 -> 0, triggers close + finalize
  let tsfn_addr = tsfn as usize;
  std::thread::spawn(move || {
    let tsfn = tsfn_addr as napi_threadsafe_function;
    unsafe {
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::release,
      );
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::release,
      );
    }
  });

  ptr::null_mut()
}

// Test napi_get_threadsafe_function_context.
// Creates a tsfn with a known context pointer, retrieves it, and verifies
// the value matches. Returns true on success, false on failure.

extern "C" fn test_tsfn_get_context(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"tsfn_get_context".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let context_value: Box<u32> = Box::new(42);
  let context_ptr = Box::into_raw(context_value) as *mut c_void;

  let mut tsfn: napi_threadsafe_function = ptr::null_mut();
  assert_napi_ok!(napi_create_threadsafe_function(
    env,
    ptr::null_mut(), // no JS func
    ptr::null_mut(), // no async resource
    resource_name,
    0,                       // unlimited queue
    1,                       // initial_thread_count
    ptr::null_mut(),         // thread_finalize_data
    None,                    // thread_finalize_cb
    context_ptr,             // context
    Some(tsfn_race_call_js), // call_js_cb
    &mut tsfn,
  ));

  // Retrieve the context and verify it matches
  let mut retrieved_context: *mut c_void = ptr::null_mut();
  assert_napi_ok!(napi_get_threadsafe_function_context(
    tsfn,
    &mut retrieved_context,
  ));

  let matches = retrieved_context == context_ptr;
  let retrieved_value = unsafe { *(retrieved_context as *const u32) };
  let value_matches = retrieved_value == 42;

  // Clean up: reclaim the box and release the tsfn
  let _ = unsafe { Box::from_raw(context_ptr as *mut u32) };
  let tsfn_addr = tsfn as usize;
  std::thread::spawn(move || {
    let tsfn = tsfn_addr as napi_threadsafe_function;
    unsafe {
      napi_release_threadsafe_function(
        tsfn,
        ThreadsafeFunctionReleaseMode::release,
      );
    }
  });

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(
    napi_get_boolean(env, matches && value_matches, &mut result,)
  );
  result
}

struct CancelBaton {
  func: napi_ref,
  task: napi_async_work,
}

unsafe extern "C" fn cancel_execute(_env: napi_env, data: *mut c_void) {
  unsafe {
    let baton: &CancelBaton = &*(data as *const CancelBaton);
    assert!(!baton.func.is_null());
    // Sleep so the cancel has a chance to arrive before execute finishes.
    std::thread::sleep(std::time::Duration::from_secs(10));
  }
}

unsafe extern "C" fn cancel_complete(
  env: napi_env,
  status: napi_status,
  data: *mut c_void,
) {
  unsafe {
    let baton: Box<CancelBaton> = Box::from_raw(data as *mut CancelBaton);
    let cancelled = status == napi_sys::Status::napi_cancelled;

    let mut global: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_global(env, &mut global));

    let mut callback: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_reference_value(env, baton.func, &mut callback));

    let mut cancelled_value: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_boolean(env, cancelled, &mut cancelled_value));

    let mut _result: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_call_function(
      env,
      global,
      callback,
      1,
      &cancelled_value,
      &mut _result
    ));
    assert_napi_ok!(napi_delete_reference(env, baton.func));
    assert_napi_ok!(napi_delete_async_work(env, baton.task));
  }
}

extern "C" fn test_cancel_async_work(
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
    "test_cancel_async_resource".as_ptr() as *const c_char,
    usize::MAX,
    &mut resource_name,
  ));

  let mut func: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[0], 1, &mut func));

  let baton = Box::new(CancelBaton {
    func,
    task: ptr::null_mut(),
  });
  let baton_ptr = Box::into_raw(baton) as *mut c_void;

  let mut async_work: napi_async_work = ptr::null_mut();
  assert_napi_ok!(napi_create_async_work(
    env,
    ptr::null_mut(),
    resource_name,
    Some(cancel_execute),
    Some(cancel_complete),
    baton_ptr,
    &mut async_work,
  ));
  let baton = unsafe { &mut *(baton_ptr as *mut CancelBaton) };
  baton.task = async_work;
  assert_napi_ok!(napi_queue_async_work(env, async_work));
  assert_napi_ok!(napi_cancel_async_work(env, async_work));

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
    napi_new_property!(env, "test_tsfn_close_race", test_tsfn_close_race),
    napi_new_property!(env, "test_tsfn_abort_race", test_tsfn_abort_race),
    napi_new_property!(
      env,
      "test_tsfn_acquire_release",
      test_tsfn_acquire_release
    ),
    napi_new_property!(env, "test_tsfn_get_context", test_tsfn_get_context),
    napi_new_property!(env, "test_cancel_async_work", test_cancel_async_work),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
