// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_void;
use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

/// Async cleanup hook callback. Called during environment teardown.
unsafe extern "C" fn async_cleanup_cb(
  _handle: napi_async_cleanup_hook_handle,
  data: *mut c_void,
) {
  let value = data as i64;
  println!("async_cleanup({})", value);
}

/// Install two async cleanup hooks. The test verifies both are
/// called during process exit (in LIFO order, like sync hooks).
extern "C" fn install_async_cleanup_hooks(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut handle1: napi_async_cleanup_hook_handle = ptr::null_mut();
  assert_napi_ok!(napi_add_async_cleanup_hook(
    env,
    Some(async_cleanup_cb),
    10 as *mut c_void,
    &mut handle1
  ));

  let mut handle2: napi_async_cleanup_hook_handle = ptr::null_mut();
  assert_napi_ok!(napi_add_async_cleanup_hook(
    env,
    Some(async_cleanup_cb),
    20 as *mut c_void,
    &mut handle2
  ));

  ptr::null_mut()
}

/// Install an async cleanup hook and then immediately remove it.
/// It should NOT be called during teardown.
extern "C" fn install_and_remove_async_cleanup_hook(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut handle: napi_async_cleanup_hook_handle = ptr::null_mut();
  assert_napi_ok!(napi_add_async_cleanup_hook(
    env,
    Some(async_cleanup_cb),
    99 as *mut c_void,
    &mut handle
  ));

  // Remove it before teardown -- the hook should not fire on exit
  assert_napi_ok!(napi_remove_async_cleanup_hook(handle));

  ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(
      env,
      "installAsyncCleanupHooks",
      install_async_cleanup_hooks
    ),
    napi_new_property!(
      env,
      "installAndRemoveAsyncCleanupHook",
      install_and_remove_async_cleanup_hook
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
