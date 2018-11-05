// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libdeno;
use std::ffi::CStr;

// Both the verson in Cargo.toml and this string must be kept in sync.
pub const DENO_VERSION: &str = "0.1.11";

pub fn get_v8_version() -> &'static str {
  let v = unsafe { libdeno::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(v) };
  c_str.to_str().unwrap()
}
