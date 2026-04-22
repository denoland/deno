// Copyright 2018-2026 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

/// Test napi_open_callback_scope / napi_close_callback_scope.
/// Opens an async context, then opens and closes a callback scope.
/// Returns true if all operations succeed.
extern "C" fn test_callback_scope(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut resource: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut resource));

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"test_cb_scope".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let mut async_context: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(
    env,
    resource,
    resource_name,
    &mut async_context,
  ));
  assert!(!async_context.is_null(), "async_context should not be null");

  let mut scope: napi_callback_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_callback_scope(
    env,
    resource,
    async_context,
    &mut scope,
  ));
  assert!(!scope.is_null(), "callback scope should not be null");

  assert_napi_ok!(napi_close_callback_scope(env, scope));
  assert_napi_ok!(napi_async_destroy(env, async_context));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test that napi_make_callback works with an async context by
/// calling a JS function through napi_make_callback.
extern "C" fn test_make_callback_with_async_context(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut resource));

  let mut resource_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"test_make_cb".as_ptr(),
    usize::MAX,
    &mut resource_name,
  ));

  let mut async_context: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(
    env,
    resource,
    resource_name,
    &mut async_context,
  ));

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_make_callback(
    env,
    async_context,
    global,
    args[0],
    0,
    ptr::null(),
    &mut result,
  ));

  assert_napi_ok!(napi_async_destroy(env, async_context));

  result
}

/// Test napi_async_init / napi_async_destroy lifecycle.
/// Verifies that async contexts can be created with or without a
/// resource object and properly destroyed.
extern "C" fn test_async_context_lifecycle(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Test 1: Create with resource object
  let mut resource: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut resource));

  let mut name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"test_resource".as_ptr(),
    usize::MAX,
    &mut name,
  ));

  let mut ctx1: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(env, resource, name, &mut ctx1));
  assert!(!ctx1.is_null());
  assert_napi_ok!(napi_async_destroy(env, ctx1));

  // Test 2: Create without resource object (null resource)
  let mut ctx2: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(env, ptr::null_mut(), name, &mut ctx2,));
  assert!(!ctx2.is_null());
  assert_napi_ok!(napi_async_destroy(env, ctx2));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test that napi_make_callback works with a real async context.
extern "C" fn test_make_callback_with_real_context(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut resource: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut resource));

  let mut name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"test_make_cb".as_ptr(),
    usize::MAX,
    &mut name,
  ));

  let mut ctx: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(env, resource, name, &mut ctx));

  let mut global: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut global));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_make_callback(
    env,
    ctx,
    global,
    args[0],
    0,
    ptr::null(),
    &mut result,
  ));

  assert_napi_ok!(napi_async_destroy(env, ctx));

  result
}

/// Ported from Node.js test_callback_scope: RunInCallbackScope.
/// Opens an async context + callback scope, calls a JS function inside
/// the scope, then cleans up.
extern "C" fn test_run_in_callback_scope(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 3);
  assert_eq!(argc, 3, "Expected 3 arguments: resource, name, callback");

  let resource = args[0];
  let resource_name = args[1];
  let callback = args[2];

  let mut async_context: napi_async_context = ptr::null_mut();
  assert_napi_ok!(napi_async_init(
    env,
    resource,
    resource_name,
    &mut async_context,
  ));

  let mut scope: napi_callback_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_callback_scope(
    env,
    resource,
    async_context,
    &mut scope,
  ));

  // Call the JS function inside the callback scope.
  // If the function throws, we still need to close the scope.
  let mut result: napi_value = ptr::null_mut();
  let call_status = unsafe {
    napi_call_function(env, resource, callback, 0, ptr::null(), &mut result)
  };

  assert_napi_ok!(napi_close_callback_scope(env, scope));
  assert_napi_ok!(napi_async_destroy(env, async_context));

  if call_status != Status::napi_ok {
    // Re-throw by returning null (exception is already pending)
    return ptr::null_mut();
  }

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_callback_scope", test_callback_scope),
    napi_new_property!(
      env,
      "test_make_callback_with_async_context",
      test_make_callback_with_async_context
    ),
    napi_new_property!(
      env,
      "test_async_context_lifecycle",
      test_async_context_lifecycle
    ),
    napi_new_property!(
      env,
      "test_make_callback_with_real_context",
      test_make_callback_with_real_context
    ),
    napi_new_property!(
      env,
      "test_run_in_callback_scope",
      test_run_in_callback_scope
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
