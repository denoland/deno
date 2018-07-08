// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
extern crate libc;

use libc::c_char;
use std::ffi::CStr;

fn string_from_ptr(ptr: *const c_char) -> String {
    let cstr = unsafe { CStr::from_ptr(ptr as *const i8) };
    String::from(cstr.to_str().unwrap())
}

#[test]
fn test_example() {
    assert_eq!(2 + 2, 4);
}

#[no_mangle]
pub extern "C" fn handle_code_fetch(
    cmd_id: u32,
    module_specifier: *const c_char,
    containing_file: *const c_char,
) {
    let module_specifier = string_from_ptr(module_specifier);
    let containing_file = string_from_ptr(containing_file);

    println!(
        "handle_code_fetch. cmd_id = {} module_specifier = {} containing_file = {}",
        cmd_id,
        module_specifier,
        containing_file
    );

    unimplemented!();
}
