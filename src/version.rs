// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno;
use std::ffi::CStr;

// This is the source of truth for the Deno version. Ignore the value in Cargo.toml.
pub const DENO_VERSION: &str = "0.1.8";

pub fn get_v8_version() -> &'static str {
  let v = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(v) };
  let version = c_str.to_str().unwrap();
  version
}
