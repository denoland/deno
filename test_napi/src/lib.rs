// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
#![allow(clippy::all)]
#![allow(clippy::undocumented_unsafe_blocks)]

use napi_sys::*;

pub mod array;
pub mod arraybuffer;
pub mod r#async;
pub mod callback;
pub mod coerce;
pub mod numbers;
pub mod object_wrap;
pub mod primitives;
pub mod promise;
pub mod properties;
pub mod strings;
pub mod typedarray;

#[macro_export]
macro_rules! get_callback_info {
  ($env: expr, $callback_info: expr, $size: literal) => {{
    let mut args = [ptr::null_mut(); $size];
    let mut argc = $size;
    let mut this = ptr::null_mut();
    unsafe {
      assert!(
        napi_get_cb_info(
          $env,
          $callback_info,
          &mut argc,
          args.as_mut_ptr(),
          &mut this,
          ptr::null_mut(),
        ) == napi_ok,
      )
    };
    (args, argc, this)
  }};
}

#[macro_export]
macro_rules! new_property {
  ($env: expr, $name: expr, $value: expr) => {
    napi_property_descriptor {
      utf8name: $name.as_ptr() as *const std::os::raw::c_char,
      name: ptr::null_mut(),
      method: Some($value),
      getter: None,
      setter: None,
      data: ptr::null_mut(),
      attributes: 0,
      value: ptr::null_mut(),
    }
  };
}

#[no_mangle]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  exports: napi_value,
) -> napi_value {
  #[cfg(windows)]
  {
    napi_sys::setup();
  }

  strings::init(env, exports);
  numbers::init(env, exports);
  typedarray::init(env, exports);
  arraybuffer::init(env, exports);
  array::init(env, exports);
  primitives::init(env, exports);
  properties::init(env, exports);
  promise::init(env, exports);
  coerce::init(env, exports);
  object_wrap::init(env, exports);
  callback::init(env, exports);
  r#async::init(env, exports);

  exports
}
