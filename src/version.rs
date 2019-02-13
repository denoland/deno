// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::libdeno;

use std::ffi::CStr;

pub const DENO: &str = env!("CARGO_PKG_VERSION");

pub fn v8() -> &'static str {
  let version = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(version) };
  c_str.to_str().unwrap()
}
