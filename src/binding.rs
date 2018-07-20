// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#![allow(dead_code)]

extern crate libc;
use libc::c_char;
use libc::c_int;
use libc::c_void;
use libc::uint32_t;

#[repr(C)]
pub struct DenoC {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct deno_buf {
    alloc_ptr: *mut u8,
    alloc_len: usize,
    data_ptr: *mut u8,
    data_len: usize,
}

type DenoRecvCb = unsafe extern "C" fn(d: *const DenoC, buf: deno_buf);

extern "C" {
    pub fn deno_init();
    pub fn deno_v8_version() -> *const c_char;
    pub fn deno_set_flags(argc: *mut c_int, argv: *mut *mut c_char);
    pub fn deno_new(data: *const c_void, cb: DenoRecvCb) -> *const DenoC;
    pub fn deno_delete(d: *const DenoC);
    pub fn deno_last_exception(d: *const DenoC) -> *const c_char;
    pub fn deno_set_response(d: *const DenoC, buf: deno_buf);
    pub fn deno_execute(
        d: *const DenoC,
        js_filename: *const c_char,
        js_source: *const c_char,
    ) -> c_int;
    pub fn deno_handle_msg_from_js(d: *const DenoC, buf: deno_buf);
    pub fn deno_reply_error(
        d: *const DenoC,
        cmd_id: uint32_t,
        msg: *const c_char,
    );
    pub fn deno_reply_null(d: *const DenoC, cmd_id: uint32_t);
    pub fn deno_reply_code_fetch(
        d: *const DenoC,
        cmd_id: uint32_t,
        module_name: *const c_char,
        filename: *const c_char,
        source_code: *const c_char,
        output_code: *const c_char,
    );
}
