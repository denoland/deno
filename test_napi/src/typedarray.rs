// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use core::ffi::c_void;
use napi_sys::Status::napi_ok;
use napi_sys::TypedarrayType::uint8_array;
use napi_sys::*;
use std::ptr;

extern "C" fn test_external(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut arraybuffer: napi_value = ptr::null_mut();
  let mut external: Box<[u8; 4]> = Box::new([0, 1, 2, 3]);
  assert!(
    unsafe {
      napi_create_external_arraybuffer(
        env,
        external.as_mut_ptr() as *mut c_void,
        external.len(),
        None,
        ptr::null_mut(),
        &mut arraybuffer,
      )
    } == napi_ok
  );

  let mut typedarray: napi_value = ptr::null_mut();
  assert!(
    unsafe {
      napi_create_typedarray(
        env,
        uint8_array,
        external.len(),
        arraybuffer,
        0,
        &mut typedarray,
      )
    } == napi_ok
  );

  std::mem::forget(external); // Leak into JS land
  typedarray
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties =
    &[crate::new_property!(env, "test_external\0", test_external)];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
