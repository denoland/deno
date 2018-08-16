// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#![allow(dead_code)]
extern crate libc;
use libc::c_char;
use libc::c_int;
use libc::c_void;

#[repr(C)]
pub struct DenoC {
  _unused: [u8; 0],
}

#[repr(C)]
#[derive(PartialEq)]
pub struct deno_buf {
  pub alloc_ptr: *mut u8,
  pub alloc_len: usize,
  pub data_ptr: *mut u8,
  pub data_len: usize,
}

type DenoRecvCb = unsafe extern "C" fn(d: *const DenoC, buf: deno_buf);

extern "C" {
  pub fn deno_init();
  pub fn deno_v8_version() -> *const c_char;
  pub fn deno_set_flags(argc: *mut c_int, argv: *mut *mut c_char);
  pub fn deno_new(data: *const c_void, cb: DenoRecvCb) -> *const DenoC;
  pub fn deno_delete(d: *const DenoC);
  pub fn deno_last_exception(d: *const DenoC) -> *const c_char;
  pub fn deno_get_data(d: *const DenoC) -> *const c_void;
  pub fn deno_set_response(d: *const DenoC, buf: deno_buf);
  pub fn deno_send(d: *const DenoC, buf: deno_buf);
  pub fn deno_execute(
    d: *const DenoC,
    js_filename: *const c_char,
    js_source: *const c_char,
  ) -> c_int;
}
