// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::ErrBox;
use std::io::{Error, ErrorKind};

#[cfg(unix)]
extern crate libc;

#[cfg(unix)]
use std::ffi::CStr;

#[cfg(unix)]
pub fn get_hostname() -> Result<String, ErrBox> {
  let buffer_len = 512; // buffer size used by Go
  let mut result_buffer = Vec::<u8>::with_capacity(buffer_len);
  let result_ptr = result_buffer.as_mut_ptr() as *mut libc::c_char;

  unsafe {
    match libc::gethostname(result_ptr, buffer_len as libc::size_t) {
      0 => Ok(CStr::from_ptr(result_ptr).to_string_lossy().into_owned()),
      _ => Err(ErrBox::from(Error::new(
        ErrorKind::Other,
        "gethostname syscall failed",
      ))),
    }
  }
}

#[cfg(windows)]
pub fn get_hostname() -> Result<String, ErrBox> {
  Err(ErrBox::from(Error::new(
    ErrorKind::Other,
    "not implemented",
  )))
}
