// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#![allow(dead_code)]
extern crate libc;
use libc::c_char;
use libc::c_int;
use libc::c_void;

#[repr(C)]
pub struct isolate_ptr {
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

type DenoRecvCb = unsafe extern "C" fn(d: *const isolate_ptr, buf: deno_buf);

extern "C" {
  pub fn deno_init();
  pub fn deno_v8_version() -> *const c_char;
  pub fn deno_set_v8_flags(argc: *mut c_int, argv: *mut *mut c_char);
  pub fn deno_new(data: *const c_void, cb: DenoRecvCb) -> *const isolate_ptr;
  pub fn deno_delete(d: *const isolate_ptr);
  pub fn deno_last_exception(d: *const isolate_ptr) -> *const c_char;
  pub fn deno_get_data(d: *const isolate_ptr) -> *const c_void;
  pub fn deno_set_response(d: *const isolate_ptr, buf: deno_buf);
  pub fn deno_send(d: *const isolate_ptr, buf: deno_buf);
  pub fn deno_execute(
    d: *const isolate_ptr,
    js_filename: *const c_char,
    js_source: *const c_char,
  ) -> c_int;
}
