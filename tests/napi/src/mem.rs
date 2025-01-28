// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr;

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_new_property;

extern "C" fn adjust_external_memory(
  env: napi_env,
  _: napi_callback_info,
) -> napi_value {
  let mut adjusted_value = 0;

  assert_napi_ok!(napi_adjust_external_memory(env, 1024, &mut adjusted_value));

  let mut result = ptr::null_mut();
  assert_napi_ok!(napi_create_int64(env, adjusted_value, &mut result));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(
    env,
    "adjust_external_memory",
    adjust_external_memory
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
