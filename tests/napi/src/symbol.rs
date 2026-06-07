// Copyright 2018-2026 the Deno authors. MIT license.

use napi_sys::ValueType::napi_string;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn symbol_new(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);

  let mut description: napi_value = std::ptr::null_mut();

  if argc >= 1 {
    let mut ty = -1;
    assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
    assert_eq!(ty, napi_string);
    description = args[0];
  }

  let mut symbol: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_symbol(env, description, &mut symbol));

  symbol
}

/// Test node_api_symbol_for (equivalent to Symbol.for()).
extern "C" fn symbol_for(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  // Get the description string
  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));
  let mut buf: Vec<u8> = vec![0; len + 1];
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    buf.as_mut_ptr() as *mut std::ffi::c_char,
    buf.len(),
    &mut len
  ));

  let mut symbol: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_symbol_for(
    env,
    buf.as_ptr() as *const std::ffi::c_char,
    len,
    &mut symbol
  ));

  symbol
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "symbolNew", symbol_new),
    napi_new_property!(env, "symbolFor", symbol_for),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
