// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

// Experimental NAPI function not yet in napi-sys crate
unsafe extern "C" {
  fn napi_create_object_with_properties(
    env: napi_env,
    prototype_or_null: napi_value,
    property_names: *const napi_value,
    property_values: *const napi_value,
    property_count: usize,
    result: *mut napi_value,
  ) -> napi_status;
}

extern "C" fn test_object_new(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut value));

  assert_napi_ok!(napi_set_element(env, value, 0, args[0]));
  assert_napi_ok!(napi_set_element(env, value, 1, args[1]));

  value
}

extern "C" fn test_object_get(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let obj = args[0];
  assert_napi_ok!(napi_set_element(env, obj, 0, args[0]));

  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_element(env, obj, 0, &mut value));
  let mut value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_element(env, obj, 1, &mut value));

  obj
}

extern "C" fn test_object_attr_property(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let obj = args[0];
  let mut property = napi_new_property!(env, "self", test_object_new);
  property.attributes = PropertyAttributes::enumerable;
  property.method = None;
  property.value = obj;
  let mut method_property = napi_new_property!(env, "method", test_object_new);
  method_property.attributes = PropertyAttributes::enumerable;
  let properties = &[property, method_property];
  assert_napi_ok!(napi_define_properties(
    env,
    obj,
    properties.len(),
    properties.as_ptr()
  ));
  obj
}

extern "C" fn test_create_object_with_properties(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut names: [napi_value; 3] = [ptr::null_mut(); 3];
  let mut values: [napi_value; 3] = [ptr::null_mut(); 3];

  // Create "name" property
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"name".as_ptr(),
    4,
    &mut names[0]
  ));
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"Foo".as_ptr(),
    3,
    &mut values[0]
  ));

  // Create "age" property
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"age".as_ptr(),
    3,
    &mut names[1]
  ));
  assert_napi_ok!(napi_create_int32(env, 42, &mut values[1]));

  // Create "active" property
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"active".as_ptr(),
    6,
    &mut names[2]
  ));
  assert_napi_ok!(napi_get_boolean(env, true, &mut values[2]));

  let mut result: napi_value = ptr::null_mut();
  let mut null_proto: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_get_null(env, &mut null_proto));

  assert_napi_ok!(napi_create_object_with_properties(
    env,
    null_proto,
    names.as_ptr(),
    values.as_ptr(),
    3,
    &mut result
  ));

  result
}

extern "C" fn test_create_object_with_properties_empty(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut result: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_object_with_properties(
    env,
    ptr::null_mut(),
    ptr::null(),
    ptr::null(),
    0,
    &mut result
  ));

  result
}

extern "C" fn test_create_object_with_custom_prototype(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Create a prototype object with a method
  let mut prototype: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut prototype));

  let mut method_name: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"test".as_ptr(),
    4,
    &mut method_name
  ));

  let mut method_func: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_function(
    env,
    c"test".as_ptr(),
    4,
    Some(test_object_new),
    ptr::null_mut(),
    &mut method_func
  ));

  assert_napi_ok!(napi_set_property(env, prototype, method_name, method_func));

  // Create object with custom prototype and a property
  let mut names: [napi_value; 1] = [ptr::null_mut(); 1];
  let mut values: [napi_value; 1] = [ptr::null_mut(); 1];

  assert_napi_ok!(napi_create_string_utf8(
    env,
    c"value".as_ptr(),
    5,
    &mut names[0]
  ));
  assert_napi_ok!(napi_create_int32(env, 42, &mut values[0]));

  let mut result: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_object_with_properties(
    env,
    prototype,
    names.as_ptr(),
    values.as_ptr(),
    1,
    &mut result
  ));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_object_new", test_object_new),
    napi_new_property!(env, "test_object_get", test_object_get),
    napi_new_property!(
      env,
      "test_object_attr_property",
      test_object_attr_property
    ),
    napi_new_property!(
      env,
      "test_create_object_with_properties",
      test_create_object_with_properties
    ),
    napi_new_property!(
      env,
      "test_create_object_with_properties_empty",
      test_create_object_with_properties_empty
    ),
    napi_new_property!(
      env,
      "test_create_object_with_custom_prototype",
      test_create_object_with_custom_prototype
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
