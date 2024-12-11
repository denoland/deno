// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use serde::Serialize;

use super::SqliteError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatementResult {
  last_insert_rowid: i64,
  changes: u64,
}

pub struct StatementSync {
  pub inner: *mut libsqlite3_sys::sqlite3_stmt,
  pub db: Rc<RefCell<Option<rusqlite::Connection>>>,
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
        return Err(SqliteError::FailedStep);
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
            return Err(SqliteError::UnknownColumnType);
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

  #[serde]
  fn run(
    &self,
    scope: &mut v8::HandleScope,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<RunStatementResult, SqliteError> {
    let raw = self.inner;
    let db = self.db.borrow();
    let db = db.as_ref().unwrap();

    let last_insert_rowid;
    let changes;

    unsafe {
      libsqlite3_sys::sqlite3_reset(raw);

      loop {
        let r = libsqlite3_sys::sqlite3_step(raw);
        if r == libsqlite3_sys::SQLITE_DONE {
          break;
        }
        if r != libsqlite3_sys::SQLITE_ROW {
          return Err(SqliteError::FailedStep);
        }
      }

      last_insert_rowid = db.last_insert_rowid();
      changes = db.changes();
    }

    Ok(RunStatementResult {
      last_insert_rowid,
      changes,
    })
  }

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
          return Err(SqliteError::FailedStep);
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
              return Err(SqliteError::UnknownColumnType);
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

  #[string]
  fn source_sql(&self) -> Result<String, SqliteError> {
    let raw = unsafe { libsqlite3_sys::sqlite3_sql(self.inner) };

    if raw.is_null() {
      return Err(SqliteError::GetSqlFailed);
    }

    let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
    let sql = cstr.to_string_lossy().to_string();

    Ok(sql)
  }

  #[string]
  fn expanded_SQL(&self) -> Result<String, SqliteError> {
    let raw = unsafe { libsqlite3_sys::sqlite3_expanded_sql(self.inner) };
    if raw.is_null() {
      return Err(SqliteError::GetSqlFailed);
    }

    let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
    let expanded_sql = cstr.to_string_lossy().to_string();

    unsafe { libsqlite3_sys::sqlite3_free(raw as *mut std::ffi::c_void) };

    Ok(expanded_sql)
  }
}
