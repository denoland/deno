// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding;
use std::ffi::CStr;

// This is the source of truth for the Deno version. Ignore the value in Cargo.toml.
const DENO_VERSION: &str = "0.0.2";

pub fn print_version() {
  let v = unsafe { binding::deno_v8_version() };
  let c_str = unsafe { CStr::from_ptr(v) };
  let version = c_str.to_str().unwrap();
  println!("deno: {}", DENO_VERSION);
  println!("v8: {}", version);
}
