// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

extern "C" fn test_get_undefined(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_get_undefined(env, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(
    env,
    "test_get_undefined",
    test_get_undefined
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
