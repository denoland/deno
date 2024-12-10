// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
  #[error(transparent)]
  SqliteError(#[from] rusqlite::Error),
  #[error("Database is already in use")]
  InUse,
  #[error(transparent)]
  Other(deno_core::error::AnyError),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseSyncOptions {
  open: bool,
  enable_foreign_key_constraints: bool,
}

pub struct DatabaseSync {
  conn: Rc<RefCell<Option<rusqlite::Connection>>>,
  options: DatabaseSyncOptions,
  location: String,
}

impl GarbageCollected for DatabaseSync {}

#[op2]
impl DatabaseSync {
  #[constructor]
  #[cppgc]
  fn new(
    #[string] location: String,
    #[serde] options: DatabaseSyncOptions,
  ) -> Result<DatabaseSync, SqliteError> {
    let db = if options.open {
      let db = rusqlite::Connection::open(&location)?;
      if options.enable_foreign_key_constraints {
        db.execute("PRAGMA foreign_keys = ON", [])?;
      }
      Some(db)
    } else {
      None
    };

    dbg!("statement typeid", std::any::TypeId::of::<StatementSync>());
    Ok(DatabaseSync {
      conn: Rc::new(RefCell::new(db)),
      location,
      options,
    })
  }

  #[fast]
  fn open(&self) -> Result<(), SqliteError> {
    let db = rusqlite::Connection::open(&self.location)?;
    if self.options.enable_foreign_key_constraints {
      db.execute("PRAGMA foreign_keys = ON", [])?;
    }

    *self.conn.borrow_mut() = Some(db);

    Ok(())
  }

  #[fast]
  fn close(&self) {}

  #[cppgc]
  fn prepare(&self, #[string] sql: &str) -> Result<StatementSync, SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let raw_handle = unsafe { db.handle() };

    let mut raw_stmt = std::ptr::null_mut();
    let r = unsafe {
      libsqlite3_sys::sqlite3_prepare_v2(
        raw_handle,
        sql.as_ptr() as *const i8,
        sql.len() as i32,
        &mut raw_stmt,
        std::ptr::null_mut(),
      )
    };

    if r != libsqlite3_sys::SQLITE_OK {
      panic!("Failed to prepare statement");
    }

    Ok(StatementSync {
      inner: raw_stmt,
      use_big_ints: false,
      allow_bare_named_params: false,
      db: self.conn.clone(),
    })
  }

  #[nofast] // divy will fix this dw
  fn exec(
    &self,
    scope: &mut v8::HandleScope,
    #[string] sql: &str,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<(), SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let mut stmt = db.prepare_cached(sql)?;
    if let Some(params) = params {
      bind(&mut stmt, scope, params, 1)?;
    }
    stmt.execute([])?;

    Ok(())
  }
}

fn bind(
  stmt: &mut rusqlite::Statement,
  scope: &mut v8::HandleScope,
  params: &v8::FunctionCallbackArguments,
  offset: usize,
) -> Result<(), SqliteError> {
  for index in offset..params.length() as usize {
    let value = params.get(index as i32);
    let index = (index + 1) - offset;
    if value.is_null() {
      // stmt.raw_bind_parameter(index, ())?;
    } else if value.is_boolean() {
      stmt.raw_bind_parameter(index, value.is_true())?;
    } else if value.is_int32() {
      stmt.raw_bind_parameter(index, value.integer_value(scope).unwrap())?;
    } else if value.is_number() {
      stmt.raw_bind_parameter(index, value.number_value(scope).unwrap())?;
    } else if value.is_big_int() {
      let bigint = value.to_big_int(scope).unwrap();
      let (value, _) = bigint.i64_value();
      stmt.raw_bind_parameter(index, value)?;
    } else if value.is_string() {
      stmt.raw_bind_parameter(index, value.to_rust_string_lossy(scope))?;
    }
    // TODO: Blobs
  }

  Ok(())
}

pub struct StatementSync {
  inner: *mut libsqlite3_sys::sqlite3_stmt,
  use_big_ints: bool,
  allow_bare_named_params: bool,
  db: Rc<RefCell<Option<rusqlite::Connection>>>,
}

impl GarbageCollected for StatementSync {}

#[op2]
impl StatementSync {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> StatementSync {
    unimplemented!()
  }

  #[fast]
  fn get(&self, #[varargs] params: Option<&v8::FunctionCallbackArguments>) {}

  #[fast]
  fn run(&self) {}

  #[fast]
  fn all(&self) {}

  #[fast]
  fn set_allowed_bare_named_parameters(&self, enabled: bool) {}

  #[fast]
  fn set_read_bigints(&self, enabled: bool) {}

  #[fast]
  fn source_sql(&self) {}

  #[string]
  fn expanded_sqlite(&self) -> Result<String, SqliteError> {
    let raw = unsafe { libsqlite3_sys::sqlite3_expanded_sql(self.inner) };
    if raw.is_null() {
      // return Err(AnyError::msg("Failed to expand SQL"));
      panic!("Failed to expand SQL");
    }

    let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
    let expanded_sql = cstr.to_string_lossy().to_string();

    unsafe { libsqlite3_sys::sqlite3_free(raw as *mut std::ffi::c_void) };

    Ok(expanded_sql)
  }
}
