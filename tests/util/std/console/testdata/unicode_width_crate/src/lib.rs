// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use libc::c_char;
use serde_json;
use std::ffi::CStr;
use unicode_width::UnicodeWidthStr;

fn from_c_char(ptr: *const c_char) -> &'static str {
  let c_str = unsafe {
    assert!(!ptr.is_null());

    CStr::from_ptr(ptr)
  };
  return c_str.to_str().unwrap();
}

#[no_mangle]
pub extern "C" fn unicode_width(json_str: *const c_char) -> usize {
  let json_str = from_c_char(json_str);

  serde_json::from_str::<String>(json_str).unwrap().width()
}
