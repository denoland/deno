// Copyright 2018-2025 the Deno authors. MIT license.

use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn get_node_global(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_global(env, &mut result));

  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties =
    &[napi_new_property!(env, "testNodeGlobal", get_node_global)];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
