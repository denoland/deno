// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::cstr;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::*;
use std::ptr;

unsafe extern "C" fn finalizer_only_callback(
  env: napi_env,
  data: *mut ::std::os::raw::c_void,
  hint: *mut ::std::os::raw::c_void,
) {
  let js_cb_ref: napi_ref = data as *mut _;
  let mut js_cb = ptr::null_mut();
  let mut undefined = ptr::null_mut();

  // TODO(bartlomieju): call equivalent to `NODE_API_CALL_RETURN_VOID` on all
  // of these
  napi_get_reference_value(env, js_cb_ref, &mut js_cb);
  napi_get_undefined(env, &mut undefined);
  napi_call_function(
    env,
    undefined,
    js_cb,
    0,
    ptr::null_mut(),
    ptr::null_mut(),
  );
  napi_delete_reference(env, js_cb_ref);
}

extern "C" fn add_finalizer_only(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, _, _) = napi_get_callback_info!(env, info, 2);

  let mut js_cb_ref: napi_ref = ptr::null_mut();
  assert_napi_ok!(napi_create_reference(env, args[1], 1, &mut js_cb_ref));
  assert_napi_ok!(napi_add_finalizer(
    env,
    args[0],
    js_cb_ref as *mut _,
    Some(finalizer_only_callback),
    ptr::null_mut(),
    ptr::null_mut(),
  ));
  std::ptr::null_mut()
}

extern "C" fn unwrap(env: napi_env, info: napi_callback_info) -> napi_value {
  let (_args, _, wrapped) = napi_get_callback_info!(env, info, 1);

  let mut data = ptr::null_mut();
  assert_napi_ok!(napi_unwrap(env, wrapped, &mut data));
  std::ptr::null_mut()
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "addFinalizerOnly", add_finalizer_only),
    napi_new_property!(env, "unwrap", unwrap),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
