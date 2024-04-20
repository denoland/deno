// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::cstr;
use napi_sys::PropertyAttributes::*;
use napi_sys::*;
use std::ptr;

static NICE: i64 = 69;

fn init_constants(env: napi_env) -> napi_value {
  let mut constants: napi_value = ptr::null_mut();
  let mut value: napi_value = ptr::null_mut();

  assert_napi_ok!(napi_create_object(env, &mut constants));
  assert_napi_ok!(napi_create_int64(env, NICE, &mut value));
  assert_napi_ok!(napi_set_named_property(
    env,
    constants,
    cstr!("nice"),
    value
  ));
  constants
}

pub fn init(env: napi_env, exports: napi_value) {
  let mut number: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_double(env, 1.0, &mut number));

  // Key name as napi_value representing `v8::String`
  let mut name_value: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("key_v8_string"),
    usize::MAX,
    &mut name_value,
  ));

  // Key symbol
  let mut symbol_description: napi_value = ptr::null_mut();
  let mut name_symbol: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    cstr!("key_v8_symbol"),
    usize::MAX,
    &mut symbol_description,
  ));
  assert_napi_ok!(napi_create_symbol(
    env,
    symbol_description,
    &mut name_symbol
  ));

  let properties = &[
    napi_property_descriptor {
      utf8name: cstr!("test_simple_property"),
      name: ptr::null_mut(),
      method: None,
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: enumerable | writable,
      value: init_constants(env),
    },
    napi_property_descriptor {
      utf8name: cstr!("test_property_rw"),
      name: ptr::null_mut(),
      method: None,
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: enumerable | writable,
      value: number,
    },
    napi_property_descriptor {
      utf8name: cstr!("test_property_r"),
      name: ptr::null_mut(),
      method: None,
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: enumerable,
      value: number,
    },
    napi_property_descriptor {
      utf8name: ptr::null(),
      name: name_value,
      method: None,
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: enumerable,
      value: number,
    },
    napi_property_descriptor {
      utf8name: ptr::null(),
      name: name_symbol,
      method: None,
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: enumerable,
      value: number,
    },
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
