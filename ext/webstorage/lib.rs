// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use libsqlite3_sys::sqlite3;
use libsqlite3_sys::sqlite3_bind_int;
use libsqlite3_sys::sqlite3_bind_text;
use libsqlite3_sys::sqlite3_column_int;
use libsqlite3_sys::sqlite3_column_text;
use libsqlite3_sys::sqlite3_column_type;
use libsqlite3_sys::sqlite3_reset;
use libsqlite3_sys::sqlite3_step;
use libsqlite3_sys::sqlite3_stmt;
use libsqlite3_sys::SQLITE_NULL;
use libsqlite3_sys::SQLITE_OPEN_CREATE;
use libsqlite3_sys::SQLITE_OPEN_MEMORY;
use libsqlite3_sys::SQLITE_OPEN_READWRITE;
use libsqlite3_sys::SQLITE_ROW;
use std::ffi::CStr;
use std::fmt;
use std::os::raw::c_int;
use std::path::PathBuf;

#[derive(Clone)]
struct OriginStorageDir(PathBuf);

const MAX_STORAGE_BYTES: u32 = 10 * 1024 * 1024;

pub fn init(origin_storage_dir: Option<PathBuf>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/webstorage",
      "01_webstorage.js",
    ))
    .ops(vec![
      op_webstorage_length::decl(),
      op_webstorage_key::decl(),
      op_webstorage_set::decl(),
      op_webstorage_get::decl(),
      op_webstorage_remove::decl(),
      op_webstorage_clear::decl(),
      op_webstorage_iterate_keys::decl(),
    ])
    .state(move |state| {
      if let Some(origin_storage_dir) = &origin_storage_dir {
        state.put(OriginStorageDir(origin_storage_dir.clone()));
      }
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webstorage.d.ts")
}

struct Storage {
  db: *mut sqlite3,
  // prepared statements
  len_stmt: *mut sqlite3_stmt,
  key_stmt: *mut sqlite3_stmt,
  size_stmt: *mut sqlite3_stmt,
  get_stmt: *mut sqlite3_stmt,
  set_stmt: *mut sqlite3_stmt,
  remove_stmt: *mut sqlite3_stmt,
  clear_stmt: *mut sqlite3_stmt,
  all_keys_stmt: *mut sqlite3_stmt,
}

impl Drop for Storage {
  fn drop(&mut self) {
    // SAFETY: This is never called twice on the same db.
    unsafe {
      libsqlite3_sys::sqlite3_close(self.db);
    }
  }
}

impl Storage {
  fn new(db: *mut sqlite3) -> Self {
    #[inline]
    fn exec(db: *mut sqlite3, str: &'static str) {
      // SAFETY: str is guaranteed to be a null terminated string.
      unsafe {
        libsqlite3_sys::sqlite3_exec(
          db,
          str.as_ptr() as *const _,
          None,
          std::ptr::null_mut(),
          std::ptr::null_mut(),
        )
      };
    }

    exec(
      db,
      "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)\0",
    );
    exec(db, PRAGMAS);

    let len_stmt = Storage::prepare(db, "SELECT COUNT(*) FROM data");
    let key_stmt =
      Storage::prepare(db, "SELECT key FROM data LIMIT 1 OFFSET ?");
    let size_stmt: *mut sqlite3_stmt = Storage::prepare(
      db,
      "SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'",
    );
    let set_stmt: *mut sqlite3_stmt = Storage::prepare(
      db,
      "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
    );
    let get_stmt: *mut sqlite3_stmt =
      Storage::prepare(db, "SELECT value FROM data WHERE key = ?");
    let remove_stmt: *mut sqlite3_stmt =
      Storage::prepare(db, "DELETE FROM data WHERE key = ?");
    let clear_stmt: *mut sqlite3_stmt =
      Storage::prepare(db, "DELETE FROM data");
    let all_keys_stmt: *mut sqlite3_stmt =
      Storage::prepare(db, "SELECT key FROM data");
    Self {
      db,
      len_stmt,
      key_stmt,
      size_stmt,
      set_stmt,
      get_stmt,
      remove_stmt,
      clear_stmt,
      all_keys_stmt,
    }
  }

  #[inline]
  fn prepare(db: *mut sqlite3, str: &'static str) -> *mut sqlite3_stmt {
    let mut stmt = std::ptr::null_mut();
    // SAFETY: db is a valid pointer to an open database.
    unsafe {
      libsqlite3_sys::sqlite3_prepare_v2(
        db,
        str.as_ptr() as *const _,
        str.len() as _,
        &mut stmt,
        std::ptr::null_mut(),
      )
    };
    stmt
  }
}

// Enable write-ahead-logging and tweak some other stuff.
const PRAGMAS: &str = "
  PRAGMA journal_mode=WAL;
  PRAGMA synchronous=NORMAL;
  PRAGMA temp_store=memory;
  PRAGMA page_size=4096;
  PRAGMA mmap_size=6000000;
  PRAGMA optimize;\0";

#[inline]
fn get_webstorage(
  state: &mut OpState,
  persistent: bool,
) -> Result<&Storage, AnyError> {
  if state.try_borrow::<Storage>().is_some() {
    return Ok(state.borrow::<Storage>());
  }

  let mut db: *mut sqlite3 = std::ptr::null_mut();
  if !persistent {
    // SAFETY: name is a null terminated string.
    unsafe {
      libsqlite3_sys::sqlite3_open_v2(
        ":memory:\0".as_ptr() as _,
        &mut db,
        SQLITE_OPEN_MEMORY | SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
        std::ptr::null_mut(),
      )
    };
  } else {
    let path = state.try_borrow::<OriginStorageDir>().ok_or_else(|| {
      DomExceptionNotSupportedError::new(
        "LocalStorage is not supported in this context.",
      )
    })?;
    std::fs::create_dir_all(&path.0)?;
    let filename =
      format!("{}\0", path.0.join("local_storage").to_string_lossy());
    // SAFETY: filename is a null terminated string.
    unsafe {
      libsqlite3_sys::sqlite3_open_v2(
        filename.as_ptr() as _,
        &mut db,
        SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
        std::ptr::null_mut(),
      )
    };
  }
  state.put(Storage::new(db));
  Ok(state.borrow::<Storage>())
}

#[inline(always)]
fn unwrap_err(code: c_int) -> Result<(), AnyError> {
  if code == libsqlite3_sys::SQLITE_OK {
    Ok(())
  } else {
    Err(type_error("Internal operation failed"))
  }
}

#[inline(always)]
fn unwrap_reset_err(code: c_int) -> Result<(), AnyError> {
  if code == libsqlite3_sys::SQLITE_DONE || code == libsqlite3_sys::SQLITE_ROW {
    Ok(())
  } else {
    Err(type_error("Internal operation failed"))
  }
}

#[op]
pub fn op_webstorage_length(
  state: &mut OpState,
  persistent: bool,
) -> Result<u32, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: len_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.len_stmt))?;
    unwrap_reset_err(sqlite3_step(conn.len_stmt))?;
    let len = sqlite3_column_int(conn.len_stmt, 0);
    Ok(len as u32)
  }
}

#[op]
pub fn op_webstorage_key(
  state: &mut OpState,
  index: u32,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: key_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.key_stmt))?;
    unwrap_err(sqlite3_bind_int(conn.key_stmt, 1, index as _))?;
    unwrap_reset_err(sqlite3_step(conn.key_stmt))?;

    if sqlite3_column_type(conn.key_stmt, 0) == SQLITE_NULL {
      Ok(None)
    } else {
      let key = sqlite3_column_text(conn.key_stmt, 0);
      let key = CStr::from_ptr(key as _).to_string_lossy().into_owned();
      Ok(Some(key))
    }
  }
}

#[op]
pub fn op_webstorage_set(
  state: &mut OpState,
  key: String,
  value: String,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: size_stmt is valid for the lifetime of the Storage.
  let size = unsafe {
    unwrap_err(sqlite3_reset(conn.size_stmt))?;
    unwrap_reset_err(sqlite3_step(conn.size_stmt))?;
    sqlite3_column_int(conn.size_stmt, 0) as u32
  };

  if size >= MAX_STORAGE_BYTES {
    return Err(
      deno_web::DomExceptionQuotaExceededError::new(
        "Exceeded maximum storage size",
      )
      .into(),
    );
  }

  // SAFETY: set_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.set_stmt))?;
    unwrap_err(sqlite3_bind_text(
      conn.set_stmt,
      1,
      key.as_ptr() as _,
      key.len() as _,
      None,
    ))?;
    unwrap_err(sqlite3_bind_text(
      conn.set_stmt,
      2,
      value.as_ptr() as _,
      value.len() as _,
      None,
    ))?;
    unwrap_reset_err(sqlite3_step(conn.set_stmt))?;
  }

  Ok(())
}

#[op]
pub fn op_webstorage_get(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: get_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.get_stmt))?;
    unwrap_err(sqlite3_bind_text(
      conn.get_stmt,
      1,
      key_name.as_ptr() as _,
      key_name.len() as _,
      None,
    ))?;
    unwrap_reset_err(sqlite3_step(conn.get_stmt))?;

    if sqlite3_column_type(conn.get_stmt, 0) == SQLITE_NULL {
      Ok(None)
    } else {
      let value = sqlite3_column_text(conn.get_stmt, 0);
      let value = CStr::from_ptr(value as _).to_string_lossy().into_owned();
      Ok(Some(value))
    }
  }
}

#[op]
pub fn op_webstorage_remove(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: remove_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.remove_stmt))?;
    unwrap_err(sqlite3_bind_text(
      conn.remove_stmt,
      1,
      key_name.as_ptr() as _,
      key_name.len() as _,
      None,
    ))?;
    unwrap_reset_err(sqlite3_step(conn.remove_stmt))?;
  }

  Ok(())
}

#[op]
pub fn op_webstorage_clear(
  state: &mut OpState,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: clear_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.clear_stmt))?;
    unwrap_reset_err(sqlite3_step(conn.clear_stmt))?;
  }

  Ok(())
}

#[op]
pub fn op_webstorage_iterate_keys(
  state: &mut OpState,
  persistent: bool,
) -> Result<Vec<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  // SAFETY: iterate_keys_stmt is valid for the lifetime of the Storage.
  unsafe {
    unwrap_err(sqlite3_reset(conn.all_keys_stmt))?;
    let mut keys = Vec::new();
    while sqlite3_step(conn.all_keys_stmt) == SQLITE_ROW {
      let key = sqlite3_column_text(conn.all_keys_stmt, 0);
      let key = CStr::from_ptr(key as _).to_string_lossy().into_owned();
      keys.push(key);
    }
    Ok(keys)
  }
}

#[derive(Debug)]
pub struct DomExceptionNotSupportedError {
  pub msg: String,
}

impl DomExceptionNotSupportedError {
  pub fn new(msg: &str) -> Self {
    DomExceptionNotSupportedError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionNotSupportedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionNotSupportedError {}

pub fn get_not_supported_error_class_name(
  e: &AnyError,
) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionNotSupportedError>()
    .map(|_| "DOMExceptionNotSupportedError")
}
