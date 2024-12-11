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

fn read_entry<'a>(
  raw: *mut libsqlite3_sys::sqlite3_stmt,
  scope: &mut v8::HandleScope<'a>,
) -> Result<Option<v8::Local<'a, v8::Object>>, SqliteError> {
  let result = v8::Object::new(scope);
  unsafe {
    let r = libsqlite3_sys::sqlite3_step(raw);

    if r == libsqlite3_sys::SQLITE_DONE {
      return Ok(None);
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
  }

  Ok(Some(result))
}

fn bind_params(
  scope: &mut v8::HandleScope,
  raw: *mut libsqlite3_sys::sqlite3_stmt,
  params: Option<&v8::FunctionCallbackArguments>,
) -> Result<(), SqliteError> {
  if let Some(params) = params {
    let len = params.length();
    for i in 0..len {
      let value = params.get(i);

      if value.is_number() {
        let value = value.number_value(scope).unwrap();
        unsafe {
          libsqlite3_sys::sqlite3_bind_double(raw, i as i32 + 1, value);
        }
      } else if value.is_string() {
        let value = value.to_rust_string_lossy(scope);
        unsafe {
          libsqlite3_sys::sqlite3_bind_text(
            raw,
            i as i32 + 1,
            value.as_ptr() as *const i8,
            value.len() as i32,
            libsqlite3_sys::SQLITE_TRANSIENT(),
          );
        }
      } else if value.is_null() {
        unsafe {
          libsqlite3_sys::sqlite3_bind_null(raw, i as i32 + 1);
        }
      } else {
        return Err(SqliteError::FailedBind("Unsupported type"));
      }
    }
  }

  Ok(())
}

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

    unsafe {
      libsqlite3_sys::sqlite3_reset(raw);
    }

    bind_params(scope, raw, params)?;

    let result = read_entry(raw, scope)
      .map(|r| r.unwrap_or_else(|| v8::Object::new(scope)))?;

    unsafe {
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

      bind_params(scope, raw, params)?;
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

      bind_params(scope, raw, params)?;
      while let Some(result) = read_entry(raw, scope)? {
        arr.push(result.into());
      }

      libsqlite3_sys::sqlite3_reset(raw);
    }

    let arr = v8::Array::new_with_elements(scope, &arr);
    Ok(arr)
  }
}
