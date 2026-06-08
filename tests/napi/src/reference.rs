// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_void;
use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

/// Test basic napi_create_reference with refcount 1 (strong),
/// napi_get_reference_value, and napi_delete_reference.
extern "C" fn test_reference_strong(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut obj: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  // Set a property so we can verify identity later
  let mut key: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"marker".as_ptr(),
    6,
    &mut key
  ));
  let mut val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 123, &mut val));
  assert_napi_ok!(napi_set_property(env, obj, key, val));

  // Create a strong reference (refcount = 1)
  let mut ref_: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, obj, 1, &mut ref_));

  // Get the value back from the reference
  let mut retrieved: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_reference_value(env, ref_, &mut retrieved));

  // Clean up
  assert_napi_ok!(napi_delete_reference(env, ref_));

  // Return the retrieved value (should be the same object)
  retrieved
}

/// Test napi_reference_ref and napi_reference_unref.
extern "C" fn test_reference_ref_unref(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut obj: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  // Create weak reference (refcount = 0)
  let mut ref_: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, obj, 0, &mut ref_));

  // Ref: 0 -> 1
  let mut count: u32 = 0;
  assert_napi_ok!(napi_reference_ref(env, ref_, &mut count));
  assert_eq!(count, 1);

  // Ref: 1 -> 2
  assert_napi_ok!(napi_reference_ref(env, ref_, &mut count));
  assert_eq!(count, 2);

  // Unref: 2 -> 1
  assert_napi_ok!(napi_reference_unref(env, ref_, &mut count));
  assert_eq!(count, 1);

  // Unref: 1 -> 0
  assert_napi_ok!(napi_reference_unref(env, ref_, &mut count));
  assert_eq!(count, 0);

  assert_napi_ok!(napi_delete_reference(env, ref_));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

/// Test napi_create_external and napi_get_value_external.
extern "C" fn test_external(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let data: Box<i64> = Box::new(42);
  let raw = Box::into_raw(data) as *mut c_void;

  let mut external: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    raw,
    Some(external_finalize),
    ptr::null_mut(),
    &mut external
  ));

  // Retrieve the data pointer
  let mut retrieved: *mut c_void = ptr::null_mut();
  assert_napi_ok!(napi_get_value_external(env, external, &mut retrieved));
  let retrieved_value = unsafe { *(retrieved as *mut i64) };
  assert_eq!(retrieved_value, 42);

  // Return the value we extracted
  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, retrieved_value as i32, &mut result));
  result
}

extern "C" fn external_finalize(
  _env: napi_env,
  data: *mut c_void,
  _hint: *mut c_void,
) {
  // Reclaim the Box to free memory
  unsafe {
    let _ = Box::from_raw(data as *mut i64);
  }
}

/// Test creating a reference to an external value.
extern "C" fn test_external_reference(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let data: Box<i64> = Box::new(99);
  let raw = Box::into_raw(data) as *mut c_void;

  let mut external: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_external(
    env,
    raw,
    Some(external_finalize),
    ptr::null_mut(),
    &mut external
  ));

  // Create a reference to the external
  let mut ref_: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, external, 1, &mut ref_));

  // Get value through reference
  let mut retrieved_val: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_reference_value(env, ref_, &mut retrieved_val));

  // Extract external data through the retrieved value
  let mut data_ptr: *mut c_void = ptr::null_mut();
  assert_napi_ok!(napi_get_value_external(env, retrieved_val, &mut data_ptr));
  let value = unsafe { *(data_ptr as *mut i64) };
  assert_eq!(value, 99);

  assert_napi_ok!(napi_delete_reference(env, ref_));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, value as i32, &mut result));
  result
}

/// Test that deleting a reference twice returns napi_generic_failure
/// (or at least does not crash / corrupt state).
extern "C" fn test_reference_double_delete(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut obj: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut ref_: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, obj, 1, &mut ref_));

  // First delete should succeed
  assert_napi_ok!(napi_delete_reference(env, ref_));

  // Second delete on the same handle -- must not crash.
  // The status may vary by implementation but the process must survive.
  unsafe {
    let _status = napi_delete_reference(env, ref_);
  }

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, true, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_reference_strong", test_reference_strong),
    napi_new_property!(
      env,
      "test_reference_ref_unref",
      test_reference_ref_unref
    ),
    napi_new_property!(env, "test_create_external", test_external),
    napi_new_property!(
      env,
      "test_create_external_reference",
      test_external_reference
    ),
    napi_new_property!(
      env,
      "test_reference_double_delete",
      test_reference_double_delete
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
