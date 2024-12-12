// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use libsqlite3_sys as ffi;
use serde::Serialize;

use super::SqliteError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatementResult {
  last_insert_rowid: i64,
  changes: u64,
}

pub struct StatementSync {
  pub inner: *mut ffi::sqlite3_stmt,
  pub db: Rc<RefCell<Option<rusqlite::Connection>>>,
}

impl Drop for StatementSync {
  fn drop(&mut self) {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // no other references to this pointer exist.
    unsafe {
      ffi::sqlite3_finalize(self.inner);
    }
  }
}

impl GarbageCollected for StatementSync {}

impl StatementSync {
  // Clear the prepared statement back to its initial state.
  fn reset(&self) {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt.
    unsafe {
      ffi::sqlite3_reset(self.inner);
    }
  }

  // Evaluate the prepared statement.
  fn step(&self) -> Result<bool, SqliteError> {
    let raw = self.inner;
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt.
    unsafe {
      let r = ffi::sqlite3_step(raw);
      if r == ffi::SQLITE_DONE {
        return Ok(true);
      }
      if r != ffi::SQLITE_ROW {
        return Err(SqliteError::FailedStep);
      }
    }

    Ok(false)
  }

  // Read the current row of the prepared statement.
  fn read_row<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<Option<v8::Local<'a, v8::Object>>, SqliteError> {
    let result = v8::Object::new(scope);
    let raw = self.inner;
    unsafe {
      if self.step()? {
        return Ok(None);
      }

      let columns = ffi::sqlite3_column_count(raw);

      for i in 0..columns {
        let name = ffi::sqlite3_column_name(raw, i);
        let name = std::ffi::CStr::from_ptr(name).to_string_lossy();
        let value = match ffi::sqlite3_column_type(raw, i) {
          ffi::SQLITE_INTEGER => {
            let value = ffi::sqlite3_column_int64(raw, i);
            v8::Integer::new(scope, value as _).into()
          }
          ffi::SQLITE_FLOAT => {
            let value = ffi::sqlite3_column_double(raw, i);
            v8::Number::new(scope, value).into()
          }
          ffi::SQLITE_TEXT => {
            let value = ffi::sqlite3_column_text(raw, i);
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
          ffi::SQLITE_BLOB => {
            let value = ffi::sqlite3_column_blob(raw, i);
            let size = ffi::sqlite3_column_bytes(raw, i);
            let value =
              std::slice::from_raw_parts(value as *const u8, size as usize);
            let value =
              v8::ArrayBuffer::new_backing_store_from_vec(value.to_vec())
                .make_shared();
            v8::ArrayBuffer::with_backing_store(scope, &value).into()
          }
          ffi::SQLITE_NULL => v8::null(scope).into(),
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
    &self,
    scope: &mut v8::HandleScope,
    params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<(), SqliteError> {
    let raw = self.inner;

    if let Some(params) = params {
      let len = params.length();
      for i in 0..len {
        let value = params.get(i);

        if value.is_number() {
          let value = value.number_value(scope).unwrap();
          unsafe {
            ffi::sqlite3_bind_double(raw, i as i32 + 1, value);
          }
        } else if value.is_string() {
          let value = value.to_rust_string_lossy(scope);
          unsafe {
            ffi::sqlite3_bind_text(
              raw,
              i as i32 + 1,
              value.as_ptr() as *const i8,
              value.len() as i32,
              ffi::SQLITE_TRANSIENT(),
            );
          }
        } else if value.is_null() {
          unsafe {
            ffi::sqlite3_bind_null(raw, i as i32 + 1);
          }
        } else if value.is_array_buffer_view() {
          let value: v8::Local<v8::ArrayBufferView> = value.try_into().unwrap();
          let data = value.data();
          let size = value.byte_length();

          unsafe {
            ffi::sqlite3_bind_blob(
              raw,
              i as i32 + 1,
              data,
              size as i32,
              ffi::SQLITE_TRANSIENT(),
            );
          }
        } else {
          return Err(SqliteError::FailedBind("Unsupported type"));
        }
      }
    }

    Ok(())
  }
}

#[op2]
impl StatementSync {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> Result<StatementSync, SqliteError> {
    Err(SqliteError::InvalidConstructor)
  }

  fn get<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    self.bind_params(scope, params)?;

    let entry = self.read_row(scope)?;
    let result = entry
      .map(|r| r.into())
      .unwrap_or_else(|| v8::undefined(scope).into());

    self.reset();

    Ok(result)
  }

  #[serde]
  fn run(
    &self,
    scope: &mut v8::HandleScope,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<RunStatementResult, SqliteError> {
    let db = self.db.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    self.bind_params(scope, params)?;
    self.step()?;

    self.reset();

    Ok(RunStatementResult {
      last_insert_rowid: db.last_insert_rowid(),
      changes: db.changes(),
    })
  }

  fn all<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Array>, SqliteError> {
    let mut arr = vec![];

    self.bind_params(scope, params)?;
    while let Some(result) = self.read_row(scope)? {
      arr.push(result.into());
    }

    self.reset();

    let arr = v8::Array::new_with_elements(scope, &arr);
    Ok(arr)
  }
}
