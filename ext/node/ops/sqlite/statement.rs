// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::v8::GetPropertyNamesArgs;
use deno_core::GarbageCollected;
use rusqlite::ffi;
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

#[derive(Debug)]
pub struct StatementSync {
  pub inner: *mut ffi::sqlite3_stmt,
  pub db: Rc<RefCell<Option<rusqlite::Connection>>>,

  pub use_big_ints: Cell<bool>,
  pub allow_bare_named_params: Cell<bool>,

  pub is_iter_finished: bool,
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
  fn reset(&self) -> Result<(), SqliteError> {
    // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    let r = unsafe { ffi::sqlite3_reset(self.inner) };

    self.check_error_code(r)
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
        self.check_error_code(r)?;
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
            v8::Number::new(scope, value as f64).into()
          } else {
            let db = self.db.borrow();
            let db = db.as_ref().ok_or(SqliteError::InUse)?;
            let handle = db.handle();

            return SqliteError::create_enhanced_error::<
              v8::Local<'a, v8::Value>,
            >(
              ffi::SQLITE_TOOBIG,
              &SqliteError::NumberTooLarge(index, value).to_string(),
              Some(handle),
            );
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
          let ab = if size == 0 {
            v8::ArrayBuffer::new(scope, 0)
          } else {
            let value =
              std::slice::from_raw_parts(value as *const u8, size as usize);
            let bs =
              v8::ArrayBuffer::new_backing_store_from_vec(value.to_vec())
                .make_shared();
            v8::ArrayBuffer::with_backing_store(scope, &bs)
          };
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

  fn bind_value(
    &self,
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
    index: i32,
  ) -> Result<(), SqliteError> {
    let raw = self.inner;
    let r = if value.is_number() {
      let value = value.number_value(scope).unwrap();

      // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      unsafe { ffi::sqlite3_bind_double(raw, index, value) }
    } else if value.is_string() {
      let value = value.to_rust_string_lossy(scope);

      // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      //
      // SQLITE_TRANSIENT is used to indicate that SQLite should make a copy of the data.
      unsafe {
        ffi::sqlite3_bind_text(
          raw,
          index,
          value.as_ptr() as *const _,
          value.len() as i32,
          ffi::SQLITE_TRANSIENT(),
        )
      }
    } else if value.is_null() {
      // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      unsafe { ffi::sqlite3_bind_null(raw, index) }
    } else if value.is_array_buffer_view() {
      let value: v8::Local<v8::ArrayBufferView> = value.try_into().unwrap();
      let mut data = value.data();
      let mut size = value.byte_length();

      // data may be NULL if length is 0 or ab is detached. we need to pass a valid pointer
      // to sqlite3_bind_blob, so we use a static empty array in this case.
      if data.is_null() {
        static EMPTY: [u8; 0] = [];

        data = EMPTY.as_ptr() as *mut _;
        size = 0;
      }

      // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      //
      // SQLITE_TRANSIENT is used to indicate that SQLite should make a copy of the data.
      unsafe {
        ffi::sqlite3_bind_blob(
          raw,
          index,
          data,
          size as i32,
          ffi::SQLITE_TRANSIENT(),
        )
      }
    } else if value.is_big_int() {
      let value: v8::Local<v8::BigInt> = value.try_into().unwrap();
      let (as_int, lossless) = value.i64_value();
      if !lossless {
        let db = self.db.borrow();
        let db = db.as_ref().ok_or(SqliteError::InUse)?;
        // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
        let handle = unsafe { db.handle() };

        return SqliteError::create_enhanced_error(
          ffi::SQLITE_TOOBIG,
          &SqliteError::FailedBind("BigInt value is too large to bind")
            .to_string(),
          Some(handle),
        );
      }

      // SAFETY: `self.inner` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      unsafe { ffi::sqlite3_bind_int64(raw, index, as_int) }
    } else {
      let db = self.db.borrow();
      let db = db.as_ref().ok_or(SqliteError::InUse)?;
      // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
      let handle = unsafe { db.handle() };

      return SqliteError::create_enhanced_error(
        ffi::SQLITE_MISMATCH,
        &SqliteError::FailedBind("Unsupported type").to_string(),
        Some(handle),
      );
    };

    self.check_error_code(r)
  }

  fn check_error_code(&self, r: i32) -> Result<(), SqliteError> {
    if r != ffi::SQLITE_OK {
      let db = self.db.borrow();
      let db = db.as_ref().ok_or(SqliteError::InUse)?;
      // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
      let handle = unsafe { db.handle() };

      // SAFETY: lifetime of the connection is guaranteed by reference
      // counting.
      let err_str = unsafe { ffi::sqlite3_errmsg(db.handle()) };

      if !err_str.is_null() {
        // SAFETY: `err_str` is a valid pointer to a null-terminated string.
        let err_str = unsafe { std::ffi::CStr::from_ptr(err_str) }
          .to_string_lossy()
          .into_owned();
        return SqliteError::create_enhanced_error(r, &err_str, Some(handle));
      }
    }

    Ok(())
  }

  // Bind the parameters to the prepared statement.
  fn bind_params(
    &self,
    scope: &mut v8::HandleScope,
    params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<(), SqliteError> {
    let raw = self.inner;
    let mut anon_start = 0;

    if let Some(params) = params {
      let param0 = params.get(0);

      if param0.is_object() && !param0.is_array_buffer_view() {
        let obj = v8::Local::<v8::Object>::try_from(param0).unwrap();
        let keys = obj
          .get_property_names(scope, GetPropertyNamesArgs::default())
          .unwrap();

        // Allow specifying named parameters without the SQLite prefix character to improve
        // ergonomics. This can be disabled with `StatementSync#setAllowBareNamedParams`.
        let mut bare_named_params = std::collections::HashMap::new();
        if self.allow_bare_named_params.get() {
          // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
          let param_count = unsafe { ffi::sqlite3_bind_parameter_count(raw) };
          for i in 1..=param_count {
            // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
            let bare_name = unsafe {
              let name = ffi::sqlite3_bind_parameter_name(raw, i);
              if name.is_null() {
                continue;
              }
              std::ffi::CStr::from_ptr(name.offset(1)).to_bytes()
            };

            let e = bare_named_params.insert(bare_name, i);
            if e.is_some() {
              let db = self.db.borrow();
              let db = db.as_ref().ok_or(SqliteError::InUse)?;
              // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
              let handle = unsafe { db.handle() };

              return SqliteError::create_enhanced_error(
                ffi::SQLITE_ERROR,
                &SqliteError::FailedBind("Duplicate named parameter")
                  .to_string(),
                Some(handle),
              );
            }
          }
        }

        let len = keys.length();
        for j in 0..len {
          let key = keys.get_index(scope, j).unwrap();
          let key_str = key.to_rust_string_lossy(scope);
          let key_c = std::ffi::CString::new(key_str).unwrap();

          // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
          let mut r = unsafe {
            ffi::sqlite3_bind_parameter_index(raw, key_c.as_ptr() as *const _)
          };
          if r == 0 {
            let lookup = bare_named_params.get(key_c.as_bytes());
            if let Some(index) = lookup {
              r = *index;
            }

            if r == 0 {
              let db = self.db.borrow();
              let db = db.as_ref().ok_or(SqliteError::InUse)?;
              // SAFETY: lifetime of the connection is guaranteed by the rusqlite API.
              let handle = unsafe { db.handle() };

              return SqliteError::create_enhanced_error(
                ffi::SQLITE_RANGE,
                &SqliteError::FailedBind("Named parameter not found")
                  .to_string(),
                Some(handle),
              );
            }
          }

          let value = obj.get(scope, key).unwrap();
          self.bind_value(scope, value, r)?;
        }

        anon_start += 1;
      }

      let mut anon_idx = 1;
      for i in anon_start..params.length() {
        // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
        while !unsafe { ffi::sqlite3_bind_parameter_name(raw, anon_idx) }
          .is_null()
        {
          anon_idx += 1;
        }

        let value = params.get(i);

        self.bind_value(scope, value, anon_idx)?;

        anon_idx += 1;
      }
    }

    Ok(())
  }
}

struct ResetGuard<'a>(&'a StatementSync);

impl Drop for ResetGuard<'_> {
  fn drop(&mut self) {
    let _ = self.0.reset();
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
    self.reset()?;

    self.bind_params(scope, params)?;

    let _reset = ResetGuard(self);

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

    let reset = ResetGuard(self);

    self.step()?;
    // Reset to return correct change metadata.
    drop(reset);

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

    let _reset = ResetGuard(self);
    while let Some(result) = self.read_row(scope)? {
      arr.push(result.into());
    }

    let arr = v8::Array::new_with_elements(scope, &arr);
    Ok(arr)
  }

  fn iterate<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Object>, SqliteError> {
    macro_rules! v8_static_strings {
      ($($ident:ident = $str:literal),* $(,)?) => {
        $(
          pub static $ident: deno_core::FastStaticString = deno_core::ascii_str!($str);
        )*
      };
    }

    v8_static_strings! {
      ITERATOR = "Iterator",
      PROTOTYPE = "prototype",
      NEXT = "next",
      RETURN = "return",
      DONE = "done",
      VALUE = "value",
    }

    self.reset()?;

    self.bind_params(scope, params)?;

    let iterate_next = |scope: &mut v8::HandleScope,
                        args: v8::FunctionCallbackArguments,
                        mut rv: v8::ReturnValue| {
      let context = v8::Local::<v8::External>::try_from(args.data())
        .expect("Iterator#next expected external data");
      // SAFETY: `context` is a valid pointer to a StatementSync instance
      let statement = unsafe { &mut *(context.value() as *mut StatementSync) };

      let names = &[
        DONE.v8_string(scope).unwrap().into(),
        VALUE.v8_string(scope).unwrap().into(),
      ];

      if statement.is_iter_finished {
        let values = &[
          v8::Boolean::new(scope, true).into(),
          v8::undefined(scope).into(),
        ];
        let null = v8::null(scope).into();
        let result =
          v8::Object::with_prototype_and_properties(scope, null, names, values);
        rv.set(result.into());
        return;
      }

      let Ok(Some(row)) = statement.read_row(scope) else {
        let _ = statement.reset();
        statement.is_iter_finished = true;

        let values = &[
          v8::Boolean::new(scope, true).into(),
          v8::undefined(scope).into(),
        ];
        let null = v8::null(scope).into();
        let result =
          v8::Object::with_prototype_and_properties(scope, null, names, values);
        rv.set(result.into());
        return;
      };

      let values = &[v8::Boolean::new(scope, false).into(), row.into()];
      let null = v8::null(scope).into();
      let result =
        v8::Object::with_prototype_and_properties(scope, null, names, values);
      rv.set(result.into());
    };

    let iterate_return = |scope: &mut v8::HandleScope,
                          args: v8::FunctionCallbackArguments,
                          mut rv: v8::ReturnValue| {
      let context = v8::Local::<v8::External>::try_from(args.data())
        .expect("Iterator#return expected external data");
      // SAFETY: `context` is a valid pointer to a StatementSync instance
      let statement = unsafe { &mut *(context.value() as *mut StatementSync) };

      statement.is_iter_finished = true;
      let _ = statement.reset();

      let names = &[
        DONE.v8_string(scope).unwrap().into(),
        VALUE.v8_string(scope).unwrap().into(),
      ];
      let values = &[
        v8::Boolean::new(scope, true).into(),
        v8::undefined(scope).into(),
      ];

      let null = v8::null(scope).into();
      let result =
        v8::Object::with_prototype_and_properties(scope, null, names, values);
      rv.set(result.into());
    };

    let external = v8::External::new(scope, self as *const _ as _);
    let next_func = v8::Function::builder(iterate_next)
      .data(external.into())
      .build(scope)
      .expect("Failed to create Iterator#next function");
    let return_func = v8::Function::builder(iterate_return)
      .data(external.into())
      .build(scope)
      .expect("Failed to create Iterator#return function");

    let global = scope.get_current_context().global(scope);
    let iter_str = ITERATOR.v8_string(scope).unwrap();
    let js_iterator: v8::Local<v8::Object> = {
      global
        .get(scope, iter_str.into())
        .unwrap()
        .try_into()
        .unwrap()
    };

    let proto_str = PROTOTYPE.v8_string(scope).unwrap();
    let js_iterator_proto = js_iterator.get(scope, proto_str.into()).unwrap();

    let names = &[
      NEXT.v8_string(scope).unwrap().into(),
      RETURN.v8_string(scope).unwrap().into(),
    ];
    let values = &[next_func.into(), return_func.into()];
    let iterator = v8::Object::with_prototype_and_properties(
      scope,
      js_iterator_proto,
      names,
      values,
    );

    Ok(iterator)
  }

  #[fast]
  fn set_allow_bare_named_parameters(&self, enabled: bool) {
    self.allow_bare_named_params.set(enabled);
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
        let db = self.db.borrow();
        let db = db.as_ref().ok_or(SqliteError::InUse)?;
        let handle = db.handle();

        return SqliteError::create_enhanced_error(
          ffi::SQLITE_ERROR,
          &SqliteError::InvalidExpandedSql.to_string(),
          Some(handle),
        );
      }
      let sql = std::ffi::CStr::from_ptr(raw as _)
        .to_string_lossy()
        .into_owned();
      ffi::sqlite3_free(raw as _);
      Ok(sql)
    }
  }
}
