// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno;
use std::ffi::CStr;

// TODO Extract this version string from Cargo.toml.
pub const DENO: &str = "0.2.1";

pub fn v8() -> &'static str {
  let version = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(version) };
  c_str.to_str().unwrap()
}
