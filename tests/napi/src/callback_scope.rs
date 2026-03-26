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

  // Deno implements callback scopes as no-ops (scopes are opened
  // automatically), so the returned scope is null. Verify the APIs
  // succeed without error.
  let mut scope: napi_callback_scope = ptr::null_mut();
  assert_napi_ok!(napi_open_callback_scope(
    env,
    resource,
    async_context,
    &mut scope,
  ));

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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_callback_scope", test_callback_scope),
    napi_new_property!(
      env,
      "test_make_callback_with_async_context",
      test_make_callback_with_async_context
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
