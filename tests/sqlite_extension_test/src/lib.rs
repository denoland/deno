// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::undocumented_unsafe_blocks)]

use std::os::raw::c_char;
use std::os::raw::c_int;

use libsqlite3_sys::*;

#[no_mangle]
pub static mut sqlite3_api: *const sqlite3_api_routines = std::ptr::null();

#[macro_export]
macro_rules! SQLITE3_EXTENSION_INIT2 {
  ($api:expr) => {
    unsafe {
      sqlite3_api = $api;
    }
  };
}

unsafe extern "C" fn test_func(
  context: *mut sqlite3_context,
  argc: c_int,
  argv: *mut *mut sqlite3_value,
) {
  if argc != 1 {
    sqlite3_result_error(
      context,
      b"test_func() requires exactly 1 argument\0".as_ptr() as *const c_char,
      -1,
    );
    return;
  }

  let arg = *argv;
  sqlite3_result_value(context, arg);
}

#[no_mangle]
pub unsafe extern "C" fn sqlite3_extension_init(
  db: *mut sqlite3,
  _pz_err_msg: *mut *mut c_char,
  p_api: *const sqlite3_api_routines,
) -> c_int {
  SQLITE3_EXTENSION_INIT2!(p_api);

  let rc = sqlite3_create_function_v2(
    db,
    b"test_func\0".as_ptr() as *const c_char,
    1,
    SQLITE_UTF8 | SQLITE_DETERMINISTIC,
    std::ptr::null_mut(),
    Some(test_func),
    None,
    None,
    None,
  );

  rc
}
