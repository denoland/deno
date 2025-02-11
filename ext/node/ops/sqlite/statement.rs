// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use libsqlite3_sys as ffi;
use serde::Serialize;

use super::SqliteError;

// ECMA-262, 15th edition, 21.1.2.6. Number.MAX_SAFE_INTEGER (2^53-1)
const MAX_SAFE_JS_INTEGER: i64 = 9007199254740991;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatementResult {
  last_insert_rowid: i64,
  changes: u64,
}

pub struct StatementSync {
  pub inner: *mut ffi::sqlite3_stmt,
  pub db: Rc<RefCell<Option<rusqlite::Connection>>>,

  pub use_big_ints: Cell<bool>,
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

struct ColumnIterator<'a> {
  stmt: &'a StatementSync,
  index: i32,
  count: i32,
}

impl<'a> ColumnIterator<'a> {
  fn new(stmt: &'a StatementSync) -> Self {
    let count = stmt.column_count();
    ColumnIterator {
      stmt,
      index: 0,
      count,
    }
  }

  fn column_count(&self) -> usize {
    self.count as usize
  }
}

impl<'a> Iterator for ColumnIterator<'a> {
  type Item = (i32, &'a [u8]);

  fn next(&mut self) -> Option<Self::Item> {
    if self.index >= self.count {
      return None;
    }

    let index = self.index;
    let name = self.stmt.column_name(self.index);

    self.index += 1;
    Some((index, name))
  }
}

impl GarbageCollected for StatementSync {}

impl StatementSync {
  // Clear the prepared statement back to its initial state.
  fn reset(&self) {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      ffi::sqlite3_reset(self.inner);
    }
  }

  // Evaluate the prepared statement.
  fn step(&self) -> Result<bool, SqliteError> {
    let raw = self.inner;
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
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

  fn column_count(&self) -> i32 {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe { ffi::sqlite3_column_count(self.inner) }
  }

  fn column_name(&self, index: i32) -> &[u8] {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      let name = ffi::sqlite3_column_name(self.inner, index);
      std::ffi::CStr::from_ptr(name as _).to_bytes()
    }
  }

  fn column_value<'a>(
    &self,
    index: i32,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      Ok(match ffi::sqlite3_column_type(self.inner, index) {
        ffi::SQLITE_INTEGER => {
          let value = ffi::sqlite3_column_int64(self.inner, index);
          if self.use_big_ints.get() {
            v8::BigInt::new_from_i64(scope, value).into()
          } else if value.abs() <= MAX_SAFE_JS_INTEGER {
            v8::Integer::new(scope, value as _).into()
          } else {
            return Err(SqliteError::NumberTooLarge(index, value));
          }
        }
        ffi::SQLITE_FLOAT => {
          let value = ffi::sqlite3_column_double(self.inner, index);
          v8::Number::new(scope, value).into()
        }
        ffi::SQLITE_TEXT => {
          let value = ffi::sqlite3_column_text(self.inner, index);
          let value = std::ffi::CStr::from_ptr(value as _);
          v8::String::new_from_utf8(
            scope,
            value.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        }
        ffi::SQLITE_BLOB => {
          let value = ffi::sqlite3_column_blob(self.inner, index);
          let size = ffi::sqlite3_column_bytes(self.inner, index);
          let value =
            std::slice::from_raw_parts(value as *const u8, size as usize);
          let bs = v8::ArrayBuffer::new_backing_store_from_vec(value.to_vec())
            .make_shared();
          let ab = v8::ArrayBuffer::with_backing_store(scope, &bs);
          v8::Uint8Array::new(scope, ab, 0, size as _).unwrap().into()
        }
        ffi::SQLITE_NULL => v8::null(scope).into(),
        _ => v8::undefined(scope).into(),
      })
    }
  }

  // Read the current row of the prepared statement.
  fn read_row<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<Option<v8::Local<'a, v8::Object>>, SqliteError> {
    if self.step()? {
      return Ok(None);
    }

    let iter = ColumnIterator::new(self);

    let num_cols = iter.column_count();

    let mut names = Vec::with_capacity(num_cols);
    let mut values = Vec::with_capacity(num_cols);

    for (index, name) in iter {
      let value = self.column_value(index, scope)?;
      let name =
        v8::String::new_from_utf8(scope, name, v8::NewStringType::Normal)
          .unwrap()
          .into();

      names.push(name);
      values.push(value);
    }

    let null = v8::null(scope).into();
    let result =
      v8::Object::with_prototype_and_properties(scope, null, &names, &values);

    Ok(Some(result))
  }

  // Bind the parameters to the prepared statement.
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

          // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
          // as it lives as long as the StatementSync instance.
          unsafe {
            ffi::sqlite3_bind_double(raw, i + 1, value);
          }
        } else if value.is_string() {
          let value = value.to_rust_string_lossy(scope);

          // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
          // as it lives as long as the StatementSync instance.
          //
          // SQLITE_TRANSIENT is used to indicate that SQLite should make a copy of the data.
          unsafe {
            ffi::sqlite3_bind_text(
              raw,
              i + 1,
              value.as_ptr() as *const _,
              value.len() as i32,
              ffi::SQLITE_TRANSIENT(),
            );
          }
        } else if value.is_null() {
          // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
          // as it lives as long as the StatementSync instance.
          unsafe {
            ffi::sqlite3_bind_null(raw, i + 1);
          }
        } else if value.is_array_buffer_view() {
          let value: v8::Local<v8::ArrayBufferView> = value.try_into().unwrap();
          let data = value.data();
          let size = value.byte_length();

          // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
          // as it lives as long as the StatementSync instance.
          //
          // SQLITE_TRANSIENT is used to indicate that SQLite should make a copy of the data.
          unsafe {
            ffi::sqlite3_bind_blob(
              raw,
              i + 1,
              data,
              size as i32,
              ffi::SQLITE_TRANSIENT(),
            );
          }
        } else if value.is_big_int() {
          let value: v8::Local<v8::BigInt> = value.try_into().unwrap();
          let (as_int, lossless) = value.i64_value();
          if !lossless {
            return Err(SqliteError::FailedBind(
              "BigInt value is too large to bind",
            ));
          }

          // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
          // as it lives as long as the StatementSync instance.
          unsafe {
            ffi::sqlite3_bind_int64(raw, i + 1, as_int);
          }
        } else {
          return Err(SqliteError::FailedBind("Unsupported type"));
        }
      }
    }

    Ok(())
  }
}

// Represents a single prepared statement. Cannot be initialized directly via constructor.
// Instances are created using `DatabaseSync#prepare`.
//
// A prepared statement is an efficient binary representation of the SQL used to create it.
#[op2]
impl StatementSync {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> Result<StatementSync, SqliteError> {
    Err(SqliteError::InvalidConstructor)
  }

  // Executes a prepared statement and returns the first result as an object.
  //
  // The prepared statement does not return any results, this method returns undefined.
  // Optionally, parameters can be bound to the prepared statement.
  fn get<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    self.reset();

    self.bind_params(scope, params)?;

    let entry = self.read_row(scope)?;
    let result = entry
      .map(|r| r.into())
      .unwrap_or_else(|| v8::undefined(scope).into());

    Ok(result)
  }

  // Executes a prepared statement and returns an object summarizing the resulting
  // changes.
  //
  // Optionally, parameters can be bound to the prepared statement.
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

  // Executes a prepared statement and returns all results as an array of objects.
  //
  // If the prepared statement does not return any results, this method returns an empty array.
  // Optionally, parameters can be bound to the prepared statement.
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

  #[fast]
  fn set_read_big_ints(&self, enabled: bool) {
    self.use_big_ints.set(enabled);
  }

  #[getter]
  #[rename("sourceSQL")]
  #[string]
  fn source_sql(&self) -> String {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      let raw = ffi::sqlite3_sql(self.inner);
      std::ffi::CStr::from_ptr(raw as _)
        .to_string_lossy()
        .into_owned()
    }
  }

  #[getter]
  #[rename("expandedSQL")]
  #[string]
  fn expanded_sql(&self) -> Result<String, SqliteError> {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      let raw = ffi::sqlite3_expanded_sql(self.inner);
      if raw.is_null() {
        return Err(SqliteError::InvalidExpandedSql);
      }
      let sql = std::ffi::CStr::from_ptr(raw as _)
        .to_string_lossy()
        .into_owned();
      ffi::sqlite3_free(raw as _);

      Ok(sql)
    }
  }
}
