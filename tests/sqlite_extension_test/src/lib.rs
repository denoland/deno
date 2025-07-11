// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::macro_metavars_in_unsafe)]

use std::os::raw::c_char;
use std::os::raw::c_int;

use rusqlite::ffi::*;

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static mut sqlite3_api: *const sqlite3_api_routines = std::ptr::null();

#[macro_export]
macro_rules! SQLITE3_EXTENSION_INIT2 {
  ($api:expr) => {
    #[allow(unused_unsafe)]
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
  unsafe {
    if argc != 1 {
      sqlite3_result_error(
        context,
        c"test_func() requires exactly 1 argument".as_ptr() as *const c_char,
        -1,
      );
      return;
    }

    let arg = *argv;
    sqlite3_result_value(context, arg);
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_extension_init(
  db: *mut sqlite3,
  _pz_err_msg: *mut *mut c_char,
  p_api: *const sqlite3_api_routines,
) -> c_int {
  unsafe {
    SQLITE3_EXTENSION_INIT2!(p_api);

    sqlite3_create_function_v2(
      db,
      c"test_func".as_ptr() as *const c_char,
      1,
      SQLITE_UTF8 | SQLITE_DETERMINISTIC,
      std::ptr::null_mut(),
      Some(test_func),
      None,
      None,
      None,
    )
  }
}
