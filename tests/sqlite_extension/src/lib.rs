// Copyright 2018-2025 the Deno authors. MIT license.

//! A simple SQLite loadable extension for testing.
//!
//! This extension provides a `test_func(x)` function that returns its argument unchanged.

use std::os::raw::c_char;
use std::os::raw::c_int;

use rusqlite::Connection;
use rusqlite::Result;
use rusqlite::ffi;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::ToSqlOutput;
use rusqlite::types::Value;

/// Entry point for SQLite to load the extension.
#[expect(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_extension_init(
  db: *mut ffi::sqlite3,
  pz_err_msg: *mut *mut c_char,
  p_api: *mut ffi::sqlite3_api_routines,
) -> c_int {
  // SAFETY: This function is called by SQLite with valid pointers.
  // extension_init2 handles the API initialization.
  unsafe { Connection::extension_init2(db, pz_err_msg, p_api, extension_init) }
}

fn extension_init(db: Connection) -> Result<bool> {
  db.create_scalar_function(
    "test_func",
    1,
    FunctionFlags::SQLITE_DETERMINISTIC,
    |ctx| {
      // Return the argument value unchanged
      let value: Value = ctx.get(0)?;
      Ok(ToSqlOutput::Owned(value))
    },
  )?;
  Ok(false)
}
