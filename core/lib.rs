// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate log;
extern crate futures;
extern crate libc;

mod flags;
mod isolate;
mod js_errors;
mod libdeno;
mod modules;
mod shared_queue;

pub use crate::flags::v8_set_flags;
pub use crate::isolate::*;
pub use crate::js_errors::*;
pub use crate::libdeno::deno_mod;
pub use crate::libdeno::PinnedBuf;
pub use crate::modules::*;

pub fn v8_version() -> &'static str {
  use std::ffi::CStr;
  let version = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(version) };
  c_str.to_str().unwrap()
}

#[test]
fn test_v8_version() {
  assert!(v8_version().len() > 3);
}
