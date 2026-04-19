// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

/// Test napi_open_handle_scope / napi_close_handle_scope.
/// Creates an object inside a handle scope, returns it via the outer scope.
extern "C" fn test_open_close_scope(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut scope: napi_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_handle_scope(env, &mut scope));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut value));

  assert_napi_ok!(napi_close_handle_scope(env, scope));

  // Return a value created in the outer (implicit) scope to prove
  // scope management does not crash.
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(env, c"ok".as_ptr(), 2, &mut result));
  result
}

/// Test napi_open_escapable_handle_scope + napi_escape_handle.
/// Creates a string inside an escapable scope, escapes it, and returns it.
extern "C" fn test_escapable_scope(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut scope: napi_escapable_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_escapable_handle_scope(env, &mut scope));

  let mut inner: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"escaped".as_ptr(),
    7,
    &mut inner
  ));

  let mut escaped: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_escape_handle(env, scope, inner, &mut escaped));

  assert_napi_ok!(napi_close_escapable_handle_scope(env, scope));

  escaped
}

/// Test that calling napi_escape_handle twice returns an error.
/// NOTE: Currently panics in Deno instead of returning
/// napi_escape_called_twice. Test is marked ignore in JS.
extern "C" fn test_escape_twice(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut scope: napi_escapable_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_escapable_handle_scope(env, &mut scope));

  let mut inner: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"first".as_ptr(),
    5,
    &mut inner
  ));

  let mut escaped: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_escape_handle(env, scope, inner, &mut escaped));

  // Second escape should fail with napi_escape_called_twice.
  let mut second: napi_value = ptr::null_mut();
  let status = unsafe { napi_escape_handle(env, scope, inner, &mut second) };
  assert_eq!(status, Status::napi_escape_called_twice);

  assert_napi_ok!(napi_close_escapable_handle_scope(env, scope));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test nested handle scopes.
extern "C" fn test_nested_scopes(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut outer: napi_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_handle_scope(env, &mut outer));

  let mut inner: napi_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_handle_scope(env, &mut inner));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));

  assert_napi_ok!(napi_close_handle_scope(env, inner));
  assert_napi_ok!(napi_close_handle_scope(env, outer));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Ported from Node.js test_handle_scope.c: NewScopeWithException.
/// Verifies that closing a handle scope still works while an exception
/// is pending.
extern "C" fn test_scope_with_exception(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let mut scope: napi_handle_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_handle_scope(env, &mut scope));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut value));

  // Get the callback argument (a function that throws)
  let mut argc: usize = 1;
  let mut exception_function: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_cb_info(
    env,
    info,
    &mut argc,
    &mut exception_function,
    ptr::null_mut(),
    ptr::null_mut()
  ));

  // Call the function that throws -- should return napi_pending_exception
  let status = unsafe {
    napi_call_function(
      env,
      value,
      exception_function,
      0,
      ptr::null(),
      ptr::null_mut(),
    )
  };
  assert_eq!(status, Status::napi_pending_exception);

  // Closing a handle scope should still work while an exception is pending
  assert_napi_ok!(napi_close_handle_scope(env, scope));
  ptr::null_mut()
}

/// Stress test: create many handles inside a scope and verify they're
/// properly scoped. Without real handle scopes, this would accumulate
/// handles unboundedly.
extern "C" fn test_handle_scope_many_handles(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Create 10000 objects inside a handle scope
  for _ in 0..100 {
    let mut scope: napi_handle_scope = ptr::null_mut();
    assert_napi_ok!(napi_open_handle_scope(env, &mut scope));
    for _ in 0..100 {
      let mut value: napi_value = ptr::null_mut();
      assert_napi_ok!(napi_create_object(env, &mut value));
    }
    assert_napi_ok!(napi_close_handle_scope(env, scope));
  }

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_open_close_scope", test_open_close_scope),
    napi_new_property!(env, "test_escapable_scope", test_escapable_scope),
    napi_new_property!(env, "test_escape_twice", test_escape_twice),
    napi_new_property!(env, "test_nested_scopes", test_nested_scopes),
    napi_new_property!(
      env,
      "test_scope_with_exception",
      test_scope_with_exception
    ),
    napi_new_property!(
      env,
      "test_handle_scope_many_handles",
      test_handle_scope_many_handles
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
