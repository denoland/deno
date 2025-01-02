// Copyright 2018-2025 the Deno authors. MIT license.

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

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(env, "symbolNew", symbol_new)];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
