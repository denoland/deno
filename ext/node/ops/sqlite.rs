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
    stmt.raw_execute()?;

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

  fn get<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Object>, SqliteError> {
    let raw = self.inner;

    let result = v8::Object::new(scope);
    unsafe {
      libsqlite3_sys::sqlite3_reset(raw);

      let r = libsqlite3_sys::sqlite3_step(raw);

      if r == libsqlite3_sys::SQLITE_DONE {
        return Ok(v8::Object::new(scope));
      }
      if r != libsqlite3_sys::SQLITE_ROW {
        // return Err(AnyError::msg("Failed to step statement"));
        return panic!("Failed to step statement");
      }

      let columns = libsqlite3_sys::sqlite3_column_count(raw);

      for i in 0..columns {
        let name = libsqlite3_sys::sqlite3_column_name(raw, i);
        let name = std::ffi::CStr::from_ptr(name).to_string_lossy().to_string();
        let value = match libsqlite3_sys::sqlite3_column_type(raw, i) {
          libsqlite3_sys::SQLITE_INTEGER => {
            let value = libsqlite3_sys::sqlite3_column_int64(raw, i);
            v8::Integer::new(scope, value as _).into()
          }
          libsqlite3_sys::SQLITE_FLOAT => {
            let value = libsqlite3_sys::sqlite3_column_double(raw, i);
            v8::Number::new(scope, value).into()
          }
          libsqlite3_sys::SQLITE_TEXT => {
            let value = libsqlite3_sys::sqlite3_column_text(raw, i);
            let value = std::ffi::CStr::from_ptr(value as _)
              .to_string_lossy()
              .to_string();
            v8::String::new_from_utf8(
              scope,
              value.as_bytes(),
              v8::NewStringType::Normal,
            )
            .unwrap()
            .into()
          }
          libsqlite3_sys::SQLITE_BLOB => {
            let value = libsqlite3_sys::sqlite3_column_blob(raw, i);
            let size = libsqlite3_sys::sqlite3_column_bytes(raw, i);
            let value =
              std::slice::from_raw_parts(value as *const u8, size as usize);
            let value =
              v8::ArrayBuffer::new_backing_store_from_vec(value.to_vec())
                .make_shared();
            v8::ArrayBuffer::with_backing_store(scope, &value).into()
          }
          libsqlite3_sys::SQLITE_NULL => v8::null(scope).into(),
          _ => {
            // return Err(AnyError::msg("Unknown column type"));
            return panic!("Unknown column type");
          }
        };

        let name = v8::String::new_from_utf8(
          scope,
          name.as_bytes(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into();
        result.set(scope, name, value);
      }

      libsqlite3_sys::sqlite3_reset(raw);
    }

    Ok(result)
  }

  #[fast]
  fn run(&self) {}

  fn all<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Array>, SqliteError> {
    let raw = self.inner;

    let mut arr = vec![];
    unsafe {
      libsqlite3_sys::sqlite3_reset(raw);
      loop {
        let result = v8::Object::new(scope);

        let r = libsqlite3_sys::sqlite3_step(raw);
        if r == libsqlite3_sys::SQLITE_DONE {
          break;
        }
        if r != libsqlite3_sys::SQLITE_ROW {
          // return Err(AnyError::msg("Failed to step statement"));
          return panic!("Failed to step statement");
        }

        let columns = libsqlite3_sys::sqlite3_column_count(raw);

        for i in 0..columns {
          let name = libsqlite3_sys::sqlite3_column_name(raw, i);
          let name =
            std::ffi::CStr::from_ptr(name).to_string_lossy().to_string();
          let value = match libsqlite3_sys::sqlite3_column_type(raw, i) {
            libsqlite3_sys::SQLITE_INTEGER => {
              let value = libsqlite3_sys::sqlite3_column_int64(raw, i);
              v8::Integer::new(scope, value as _).into()
            }
            libsqlite3_sys::SQLITE_FLOAT => {
              let value = libsqlite3_sys::sqlite3_column_double(raw, i);
              v8::Number::new(scope, value).into()
            }
            libsqlite3_sys::SQLITE_TEXT => {
              let value = libsqlite3_sys::sqlite3_column_text(raw, i);
              let value = std::ffi::CStr::from_ptr(value as _)
                .to_string_lossy()
                .to_string();
              v8::String::new_from_utf8(
                scope,
                value.as_bytes(),
                v8::NewStringType::Normal,
              )
              .unwrap()
              .into()
            }
            libsqlite3_sys::SQLITE_BLOB => {
              let value = libsqlite3_sys::sqlite3_column_blob(raw, i);
              let size = libsqlite3_sys::sqlite3_column_bytes(raw, i);
              let value =
                std::slice::from_raw_parts(value as *const u8, size as usize);
              let value =
                v8::ArrayBuffer::new_backing_store_from_vec(value.to_vec())
                  .make_shared();
              v8::ArrayBuffer::with_backing_store(scope, &value).into()
            }
            libsqlite3_sys::SQLITE_NULL => v8::null(scope).into(),
            _ => {
              // return Err(AnyError::msg("Unknown column type"));
              return panic!("Unknown column type");
            }
          };

          let name = v8::String::new_from_utf8(
            scope,
            name.as_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into();
          result.set(scope, name, value);
        }

        arr.push(result.into());
      }

      libsqlite3_sys::sqlite3_reset(raw);
    }

    let arr = v8::Array::new_with_elements(scope, &arr);
    Ok(arr)
  }

  #[fast]
  fn set_allowed_bare_named_parameters(&self, enabled: bool) {}

  #[fast]
  fn set_read_bigints(&self, enabled: bool) {}

  #[string]
  fn source_sql(&self) -> Result<String, SqliteError> {
    let raw = unsafe { libsqlite3_sys::sqlite3_sql(self.inner) };

    if raw.is_null() {
      // return Err(AnyError::msg("Failed to get SQL"));
      panic!("Failed to get SQL");
    }

    let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
    let sql = cstr.to_string_lossy().to_string();

    Ok(sql)
  }

  #[string]
  fn expanded_sql(&self) -> Result<String, SqliteError> {
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
