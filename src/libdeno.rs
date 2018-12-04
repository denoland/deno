// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use libc::c_char;
use libc::c_int;
use libc::c_void;
use std::ops::{Deref, DerefMut};
use std::ptr::null;

// TODO(F001): change this definition to `extern { pub type isolate; }`
// After RFC 1861 is stablized. See https://github.com/rust-lang/rust/issues/43467.
#[repr(C)]
pub struct isolate {
  _unused: [u8; 0],
}

/// If "alloc_ptr" is not null, this type represents a buffer which is created
/// in C side, and then passed to Rust side by `DenoRecvCb`. Finally it should
/// be moved back to C side by `deno_respond`. If it is not passed to
/// `deno_respond` in the end, it will be leaked.
///
/// If "alloc_ptr" is null, this type represents a borrowed slice.
#[repr(C)]
pub struct deno_buf {
  alloc_ptr: *const u8,
  alloc_len: usize,
  data_ptr: *const u8,
  data_len: usize,
}

/// `deno_buf` can not clone, and there is no interior mutability.
/// This type satisfies Send bound.
unsafe impl Send for deno_buf {}

impl deno_buf {
  pub fn empty() -> Self {
    Self {
      alloc_ptr: null(),
      alloc_len: 0,
      data_ptr: null(),
      data_len: 0,
    }
  }

  pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Self {
    Self {
      alloc_ptr: null(),
      alloc_len: 0,
      data_ptr: ptr,
      data_len: len,
    }
  }
}

/// Converts Rust &Buf to libdeno `deno_buf`.
impl<'a> From<&'a [u8]> for deno_buf {
  fn from(x: &'a [u8]) -> Self {
    Self {
      alloc_ptr: null(),
      alloc_len: 0,
      data_ptr: x.as_ref().as_ptr(),
      data_len: x.len(),
    }
  }
}

impl Deref for deno_buf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    unsafe { std::slice::from_raw_parts(self.data_ptr, self.data_len) }
  }
}

impl DerefMut for deno_buf {
  fn deref_mut(&mut self) -> &mut [u8] {
    unsafe {
      std::slice::from_raw_parts_mut(self.data_ptr as *mut u8, self.data_len)
    }
  }
}

impl AsRef<[u8]> for deno_buf {
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

impl AsMut<[u8]> for deno_buf {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}

type DenoRecvCb = unsafe extern "C" fn(
  user_data: *mut c_void,
  req_id: i32,
  buf: deno_buf,
  data_buf: deno_buf,
);

#[repr(C)]
pub struct deno_config {
  pub shared: deno_buf,
  pub recv_cb: DenoRecvCb,
}

extern "C" {
  pub fn deno_init();
  pub fn deno_v8_version() -> *const c_char;
  pub fn deno_set_v8_flags(argc: *mut c_int, argv: *mut *mut c_char);
  pub fn deno_new(snapshot: deno_buf, config: deno_config) -> *const isolate;
  pub fn deno_delete(i: *const isolate);
  pub fn deno_last_exception(i: *const isolate) -> *const c_char;
  pub fn deno_check_promise_errors(i: *const isolate);
  pub fn deno_respond(
    i: *const isolate,
    user_data: *mut c_void,
    req_id: i32,
    buf: deno_buf,
  );
  pub fn deno_execute(
    i: *const isolate,
    user_data: *mut c_void,
    js_filename: *const c_char,
    js_source: *const c_char,
  ) -> c_int;
}
