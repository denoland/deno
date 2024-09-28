// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::all)]
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::undocumented_unsafe_blocks)]

use std::ffi::c_void;

use napi_sys::*;

pub mod array;
pub mod arraybuffer;
pub mod r#async;
pub mod bigint;
pub mod callback;
pub mod coerce;
pub mod date;
pub mod env;
pub mod error;
pub mod finalizer;
pub mod make_callback;
pub mod mem;
pub mod numbers;
pub mod object;
pub mod object_wrap;
pub mod primitives;
pub mod promise;
pub mod properties;
pub mod strings;
pub mod symbol;
pub mod tsfn;
pub mod typedarray;
pub mod uv;

#[macro_export]
macro_rules! cstr {
  ($s: literal) => {{
    std::ffi::CString::new($s).unwrap().into_raw()
  }};
}

#[macro_export]
macro_rules! assert_napi_ok {
  ($call: expr) => {{
    assert_eq!(unsafe { $call }, napi_sys::Status::napi_ok);
  }};
}

#[macro_export]
macro_rules! napi_get_callback_info {
  ($env: expr, $callback_info: expr, $size: literal) => {{
    let mut args = [std::ptr::null_mut(); $size];
    let mut argc = $size;
    let mut this = std::ptr::null_mut();
    crate::assert_napi_ok!(napi_get_cb_info(
      $env,
      $callback_info,
      &mut argc,
      args.as_mut_ptr(),
      &mut this,
      std::ptr::null_mut(),
    ));
    (args, argc, this)
  }};
}

#[macro_export]
macro_rules! napi_new_property {
  ($env: expr, $name: expr, $value: expr) => {
    napi_property_descriptor {
      utf8name: concat!($name, "\0").as_ptr() as *const std::os::raw::c_char,
      name: std::ptr::null_mut(),
      method: Some($value),
      getter: None,
      setter: None,
      data: std::ptr::null_mut(),
      attributes: 0,
      value: std::ptr::null_mut(),
    }
  };
}

extern "C" fn cleanup(arg: *mut c_void) {
  println!("cleanup({})", arg as i64);
}

extern "C" fn remove_this_hook(arg: *mut c_void) {
  let env = arg as napi_env;
  unsafe { napi_remove_env_cleanup_hook(env, Some(remove_this_hook), arg) };
}

static SECRET: i64 = 42;
static WRONG_SECRET: i64 = 17;
static THIRD_SECRET: i64 = 18;

extern "C" fn install_cleanup_hook(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 0);

  unsafe {
    napi_add_env_cleanup_hook(env, Some(cleanup), WRONG_SECRET as *mut c_void);
    napi_add_env_cleanup_hook(env, Some(cleanup), SECRET as *mut c_void);
    napi_add_env_cleanup_hook(env, Some(cleanup), THIRD_SECRET as *mut c_void);
    napi_add_env_cleanup_hook(env, Some(remove_this_hook), env as *mut c_void);
    napi_remove_env_cleanup_hook(
      env,
      Some(cleanup),
      WRONG_SECRET as *mut c_void,
    );
  }

  std::ptr::null_mut()
}

pub fn init_cleanup_hook(env: napi_env, exports: napi_value) {
  let properties = &[napi_new_property!(
    env,
    "installCleanupHook",
    install_cleanup_hook
  )];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}

#[no_mangle]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  _: napi_value,
) -> napi_value {
  #[cfg(windows)]
  {
    napi_sys::setup();
    libuv_sys_lite::setup();
  }

  // We create a fresh exports object and leave the passed
  // exports object empty.
  //
  // https://github.com/denoland/deno/issues/17349
  let mut exports = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut exports));

  strings::init(env, exports);
  numbers::init(env, exports);
  typedarray::init(env, exports);
  arraybuffer::init(env, exports);
  array::init(env, exports);
  env::init(env, exports);
  error::init(env, exports);
  finalizer::init(env, exports);
  primitives::init(env, exports);
  properties::init(env, exports);
  promise::init(env, exports);
  coerce::init(env, exports);
  object_wrap::init(env, exports);
  callback::init(env, exports);
  r#async::init(env, exports);
  date::init(env, exports);
  tsfn::init(env, exports);
  mem::init(env, exports);
  bigint::init(env, exports);
  symbol::init(env, exports);
  make_callback::init(env, exports);
  object::init(env, exports);
  uv::init(env, exports);

  init_cleanup_hook(env, exports);

  exports
}
