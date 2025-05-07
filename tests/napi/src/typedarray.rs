// Copyright 2018-2025 the Deno authors. MIT license.

use core::ffi::c_void;
use std::os::raw::c_char;
use std::ptr;

use napi_sys::Status::napi_ok;
use napi_sys::TypedarrayType;
use napi_sys::ValueType::napi_number;
use napi_sys::ValueType::napi_object;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn test_multiply(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 2);
  assert_eq!(argc, 2);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_object);

  let input_array = args[0];
  let mut is_typed_array = false;
  assert!(
    unsafe { napi_is_typedarray(env, input_array, &mut is_typed_array) }
      == napi_ok
  );

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[1], &mut ty));
  assert_eq!(ty, napi_number);

  let mut multiplier: f64 = 0.0;
  assert_napi_ok!(napi_get_value_double(env, args[1], &mut multiplier));

  let mut ty = -1;
  let mut input_buffer = ptr::null_mut();
  let mut byte_offset = 0;
  let mut length = 0;

  assert_napi_ok!(napi_get_typedarray_info(
    env,
    input_array,
    &mut ty,
    &mut length,
    ptr::null_mut(),
    &mut input_buffer,
    &mut byte_offset,
  ));

  let mut data = ptr::null_mut();
  let mut byte_length = 0;

  assert_napi_ok!(napi_get_arraybuffer_info(
    env,
    input_buffer,
    &mut data,
    &mut byte_length
  ));

  let mut output_buffer = ptr::null_mut();
  let mut output_ptr = ptr::null_mut();
  assert_napi_ok!(napi_create_arraybuffer(
    env,
    byte_length,
    &mut output_ptr,
    &mut output_buffer,
  ));

  let mut output_array: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_typedarray(
    env,
    ty,
    length,
    output_buffer,
    byte_offset,
    &mut output_array,
  ));

  if ty == TypedarrayType::uint8_array {
    let input_bytes = unsafe { (data as *mut u8).offset(byte_offset as isize) };
    let output_bytes = output_ptr as *mut u8;
    for i in 0..length {
      unsafe {
        *output_bytes.offset(i as isize) =
          (*input_bytes.offset(i as isize) as f64 * multiplier) as u8;
      }
    }
  } else if ty == TypedarrayType::float64_array {
    let input_doubles =
      unsafe { (data as *mut f64).offset(byte_offset as isize) };
    let output_doubles = output_ptr as *mut f64;
    for i in 0..length {
      unsafe {
        *output_doubles.offset(i as isize) =
          *input_doubles.offset(i as isize) * multiplier;
      }
    }
  } else {
    assert_napi_ok!(napi_throw_error(
      env,
      ptr::null(),
      "Typed array was of a type not expected by test.".as_ptr()
        as *const c_char,
    ));
    return ptr::null_mut();
  }

  output_array
}

extern "C" fn test_external(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut arraybuffer: napi_value = ptr::null_mut();
  let mut external: Box<[u8; 4]> = Box::new([0, 1, 2, 3]);
  assert_napi_ok!(napi_create_external_arraybuffer(
    env,
    external.as_mut_ptr() as *mut c_void,
    external.len(),
    None,
    ptr::null_mut(),
    &mut arraybuffer,
  ));

  let mut typedarray: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_typedarray(
    env,
    TypedarrayType::uint8_array,
    external.len(),
    arraybuffer,
    0,
    &mut typedarray,
  ));

  std::mem::forget(external); // Leak into JS land
  typedarray
}

extern "C" fn test_is_buffer(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut is_buffer: bool = false;
  assert_napi_ok!(napi_is_buffer(env, args[0], &mut is_buffer));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, is_buffer, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_external", test_external),
    napi_new_property!(env, "test_multiply", test_multiply),
    napi_new_property!(env, "test_is_buffer", test_is_buffer),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
