// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

use deno_core::GarbageCollected;
use deno_core::ToV8;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::GetPropertyNamesArgs;
use deno_core::v8_static_strings;
use rusqlite::ffi;

use super::SqliteError;
use super::validators;

// ECMA-262, 15th edition, 21.1.2.6. Number.MAX_SAFE_INTEGER (2^53-1)
const MAX_SAFE_JS_INTEGER: i64 = 9007199254740991;

pub struct RunStatementResult {
  last_insert_rowid: i64,
  changes: u64,
  use_big_ints: bool,
}

impl<'a> ToV8<'a> for RunStatementResult {
  type Error = SqliteError;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    v8_static_strings! {
      LAST_INSERT_ROW_ID = "lastInsertRowid",
      CHANGES = "changes",
    }

    let obj = v8::Object::new(scope);

    let last_insert_row_id_str = LAST_INSERT_ROW_ID.v8_string(scope).unwrap();
    let last_insert_row_id = if self.use_big_ints {
      v8::BigInt::new_from_i64(scope, self.last_insert_rowid).into()
    } else {
      v8::Number::new(scope, self.last_insert_rowid as f64).into()
    };

    obj
      .set(scope, last_insert_row_id_str.into(), last_insert_row_id)
      .unwrap();

    let changes_str = CHANGES.v8_string(scope).unwrap();
    let changes = if self.use_big_ints {
      v8::BigInt::new_from_u64(scope, self.changes).into()
    } else {
      v8::Number::new(scope, self.changes as f64).into()
    };

    obj.set(scope, changes_str.into(), changes).unwrap();
    Ok(obj.into())
  }
}

pub type InnerStatementPtr = Rc<Cell<Option<*mut ffi::sqlite3_stmt>>>;

#[derive(Debug)]
pub struct StatementSync {
  pub inner: InnerStatementPtr,
  pub db: Weak<RefCell<Option<rusqlite::Connection>>>,
  pub statements: Rc<RefCell<Vec<InnerStatementPtr>>>,
  pub ignore_next_sqlite_error: Rc<Cell<bool>>,

  pub use_big_ints: Cell<bool>,
  pub allow_bare_named_params: Cell<bool>,
  pub allow_unknown_named_params: Cell<bool>,

  pub is_iter_finished: Cell<bool>,
}

impl Drop for StatementSync {
  fn drop(&mut self) {
    let mut statements = self.statements.borrow_mut();
    let mut finalized_stmt = None;

    if let Some(pos) = statements
      .iter()
      .position(|stmt| Rc::ptr_eq(stmt, &self.inner))
    {
      let stmt = statements.remove(pos);
      finalized_stmt = stmt.get();
      stmt.set(None);
    }

    if let Some(ptr) = finalized_stmt {
      // SAFETY: `ptr` is a valid pointer to a sqlite3_stmt.
      unsafe {
        ffi::sqlite3_finalize(ptr);
      }
    }
  }
}

pub(crate) fn check_error_code(
  r: i32,
  db: *mut ffi::sqlite3,
) -> Result<(), SqliteError> {
  if r != ffi::SQLITE_OK {
    // SAFETY: lifetime of the connection is guaranteed by reference
    // counting.
    let err_message = unsafe { ffi::sqlite3_errmsg(db) };

    if !err_message.is_null() {
      // SAFETY: `err_msg` is a valid pointer to a null-terminated string.
      let err_message = unsafe { std::ffi::CStr::from_ptr(err_message) }
        .to_string_lossy()
        .into_owned();
      // SAFETY: `err_str` is a valid pointer to a null-terminated string.
      let err_str = unsafe { std::ffi::CStr::from_ptr(ffi::sqlite3_errstr(r)) }
        .to_string_lossy()
        .into_owned();

      return Err(SqliteError::SqliteSysError {
        message: err_message,
        errcode: r as _,
        errstr: err_str,
      });
    }
  }

  Ok(())
}

pub(crate) fn check_error_code2(r: i32) -> Result<(), SqliteError> {
  // SAFETY: `ffi::sqlite3_errstr` is a valid function that returns a pointer to a null-terminated
  // string.
  let err_str = unsafe { std::ffi::CStr::from_ptr(ffi::sqlite3_errstr(r)) }
    .to_string_lossy()
    .into_owned();

  Err(SqliteError::SqliteSysError {
    message: err_str.clone(),
    errcode: r as _,
    errstr: err_str,
  })
}

struct ColumnIterator<'a> {
  stmt: &'a StatementSync,
  index: i32,
  count: i32,
}

impl<'a> ColumnIterator<'a> {
  fn new(stmt: &'a StatementSync) -> Result<Self, SqliteError> {
    let count = stmt.column_count()?;
    Ok(ColumnIterator {
      stmt,
      index: 0,
      count,
    })
  }

  fn column_count(&self) -> usize {
    self.count as usize
  }
}

impl<'a> Iterator for ColumnIterator<'a> {
  type Item = Result<(i32, &'a [u8]), SqliteError>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.index >= self.count {
      return None;
    }

    let index = self.index;
    let name = match self.stmt.column_name(index) {
      Ok(name) => name,
      Err(_) => return Some(Err(SqliteError::StatementFinalized)),
    };

    self.index += 1;
    Some(Ok((index, name)))
  }
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for StatementSync {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"StatementSync"
  }
}

impl StatementSync {
  fn stmt_ptr(&self) -> Result<*mut ffi::sqlite3_stmt, SqliteError> {
    let ptr = self.inner.get();
    match ptr {
      Some(p) => Ok(p),
      None => Err(SqliteError::StatementFinalized),
    }
  }

  fn assert_statement_finalized(&self) -> Result<(), SqliteError> {
    if self.inner.get().is_none() {
      return Err(SqliteError::StatementFinalized);
    }
    Ok(())
  }

  // Clear the prepared statement back to its initial state.
  fn reset(&self) -> Result<(), SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    let r = unsafe { ffi::sqlite3_reset(raw) };

    self.check_error_code(r)
  }

  // Evaluate the prepared statement.
  fn step(&self) -> Result<bool, SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
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

  fn column_count(&self) -> Result<i32, SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    let count = unsafe { ffi::sqlite3_column_count(raw) };
    Ok(count)
  }

  fn column_name(&self, index: i32) -> Result<&[u8], SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      let name = ffi::sqlite3_column_name(raw, index);
      Ok(std::ffi::CStr::from_ptr(name as _).to_bytes())
    }
  }

  fn column_value<'a>(
    &self,
    index: i32,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      Ok(match ffi::sqlite3_column_type(raw, index) {
        ffi::SQLITE_INTEGER => {
          let value = ffi::sqlite3_column_int64(raw, index);
          if self.use_big_ints.get() {
            v8::BigInt::new_from_i64(scope, value).into()
          } else if value.abs() <= MAX_SAFE_JS_INTEGER {
            v8::Number::new(scope, value as f64).into()
          } else {
            return Err(SqliteError::NumberTooLarge(index, value));
          }
        }
        ffi::SQLITE_FLOAT => {
          let value = ffi::sqlite3_column_double(raw, index);
          v8::Number::new(scope, value).into()
        }
        ffi::SQLITE_TEXT => {
          let value = ffi::sqlite3_column_text(raw, index);
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
          let value = ffi::sqlite3_column_blob(raw, index);
          let size = ffi::sqlite3_column_bytes(raw, index);
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
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<Option<v8::Local<'a, v8::Object>>, SqliteError> {
    if self.step()? {
      return Ok(None);
    }

    let iter = ColumnIterator::new(self)?;

    let num_cols = iter.column_count();

    let mut names = Vec::with_capacity(num_cols);
    let mut values = Vec::with_capacity(num_cols);

    for item in iter {
      let (index, name) = item?;
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
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<v8::Value>,
    index: i32,
  ) -> Result<(), SqliteError> {
    let raw = self.stmt_ptr()?;
    let r = if value.is_number() {
      let value = value.number_value(scope).unwrap();

      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      unsafe { ffi::sqlite3_bind_double(raw, index, value) }
    } else if value.is_string() {
      let value = value.to_rust_string_lossy(scope);

      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
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
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
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

      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
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
        return Err(SqliteError::InvalidBindValue(
          "BigInt value is too large to bind",
        ));
      }

      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      // as it lives as long as the StatementSync instance.
      unsafe { ffi::sqlite3_bind_int64(raw, index, as_int) }
    } else {
      return Err(SqliteError::InvalidBindType(index));
    };

    self.check_error_code(r)
  }

  fn check_error_code(&self, r: i32) -> Result<(), SqliteError> {
    if r != ffi::SQLITE_OK {
      if self.ignore_next_sqlite_error.get() {
        self.ignore_next_sqlite_error.set(false);
        return Ok(());
      }
      let db_rc = self.db.upgrade().ok_or(SqliteError::InUse)?;
      let db = db_rc.borrow();
      let db = db.as_ref().ok_or(SqliteError::InUse)?;

      // SAFETY: db.handle() is valid
      unsafe {
        check_error_code(r, db.handle())?;
      }
    }

    Ok(())
  }

  // Bind the parameters to the prepared statement.
  fn bind_params(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<(), SqliteError> {
    let raw = self.stmt_ptr()?;
    // Reset the prepared statement to its initial state.
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    unsafe {
      let r = ffi::sqlite3_clear_bindings(raw);
      self.check_error_code(r)?;
    }

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
            let full_name = unsafe {
              let name = ffi::sqlite3_bind_parameter_name(raw, i);
              if name.is_null() {
                continue;
              }
              std::ffi::CStr::from_ptr(name).to_bytes()
            };
            let bare_name = &full_name[1..];

            let e = bare_named_params.insert(bare_name, i);
            if let Some(existing_index) = e {
              let bare_name_str = std::str::from_utf8(bare_name)?;
              let full_name_str = std::str::from_utf8(full_name)?;

              // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
              unsafe {
                let existing_full_name =
                  ffi::sqlite3_bind_parameter_name(raw, existing_index);
                let existing_full_name_str =
                  std::ffi::CStr::from_ptr(existing_full_name).to_str()?;

                return Err(SqliteError::DuplicateNamedParameter(
                  bare_name_str.to_string(),
                  existing_full_name_str.to_string(),
                  full_name_str.to_string(),
                ));
              }
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
              if self.allow_unknown_named_params.get() {
                continue;
              }

              return Err(SqliteError::UnknownNamedParameter(
                key_c.into_string().unwrap(),
              ));
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
  #[reentrant]
  fn get<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
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
  #[to_v8]
  fn run(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<RunStatementResult, SqliteError> {
    let db_rc = self.db.upgrade().ok_or(SqliteError::InUse)?;
    let db = db_rc.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    self.bind_params(scope, params)?;

    let reset = ResetGuard(self);

    self.step()?;
    // Reset to return correct change metadata.
    drop(reset);

    Ok(RunStatementResult {
      last_insert_rowid: db.last_insert_rowid(),
      changes: db.changes(),
      use_big_ints: self.use_big_ints.get(),
    })
  }

  // Executes a prepared statement and returns all results as an array of objects.
  //
  // If the prepared statement does not return any results, this method returns an empty array.
  // Optionally, parameters can be bound to the prepared statement.
  fn all<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
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
    scope: &mut v8::PinScope<'a, '_>,
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

    let iterate_next = |scope: &mut v8::PinScope<'_, '_>,
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

      if statement.is_iter_finished.get() {
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
        statement.is_iter_finished.set(true);

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

    let iterate_return = |scope: &mut v8::PinScope<'_, '_>,
                          args: v8::FunctionCallbackArguments,
                          mut rv: v8::ReturnValue| {
      let context = v8::Local::<v8::External>::try_from(args.data())
        .expect("Iterator#return expected external data");
      // SAFETY: `context` is a valid pointer to a StatementSync instance
      let statement = unsafe { &mut *(context.value() as *mut StatementSync) };

      statement.is_iter_finished.set(true);
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

    self.is_iter_finished.set(false);

    Ok(iterator)
  }

  #[fast]
  #[undefined]
  fn set_allow_bare_named_parameters(
    &self,
    #[validate(validators::allow_bare_named_params_bool)] enabled: bool,
  ) -> Result<(), SqliteError> {
    self.assert_statement_finalized()?;
    self.allow_bare_named_params.set(enabled);
    Ok(())
  }

  #[fast]
  #[undefined]
  fn set_allow_unknown_named_parameters(
    &self,
    #[validate(validators::allow_unknown_named_params_bool)] enabled: bool,
  ) -> Result<(), SqliteError> {
    self.assert_statement_finalized()?;
    self.allow_unknown_named_params.set(enabled);
    Ok(())
  }

  #[fast]
  #[undefined]
  fn set_read_big_ints(
    &self,
    #[validate(validators::read_big_ints_bool)] enabled: bool,
  ) -> Result<(), SqliteError> {
    self.assert_statement_finalized()?;
    self.use_big_ints.set(enabled);
    Ok(())
  }

  #[getter]
  #[rename("sourceSQL")]
  #[string]
  fn source_sql(&self) -> Result<String, SqliteError> {
    let inner = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    let source_sql = unsafe {
      let raw = ffi::sqlite3_sql(inner);
      std::ffi::CStr::from_ptr(raw as _)
        .to_string_lossy()
        .into_owned()
    };
    Ok(source_sql)
  }

  #[getter]
  #[rename("expandedSQL")]
  #[string]
  fn expanded_sql(&self) -> Result<String, SqliteError> {
    let inner = self.stmt_ptr()?;
    // SAFETY: `inner` is a valid pointer to a sqlite3_stmt
    // as it lives as long as the StatementSync instance.
    unsafe {
      let raw = ffi::sqlite3_expanded_sql(inner);
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

  fn columns<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Array>, SqliteError> {
    v8_static_strings! {
      NAME = "name",
      COLUMN = "column",
      TABLE = "table",
      DATABASE = "database",
      TYPE = "type",
    }

    let column_count = self.column_count()?;
    let mut columns = Vec::with_capacity(column_count as usize);

    // Pre-create property keys
    let name_key = NAME.v8_string(scope).unwrap().into();
    let column_key = COLUMN.v8_string(scope).unwrap().into();
    let table_key = TABLE.v8_string(scope).unwrap().into();
    let database_key = DATABASE.v8_string(scope).unwrap().into();
    let type_key = TYPE.v8_string(scope).unwrap().into();

    let keys = &[name_key, column_key, table_key, database_key, type_key];
    let raw = self.stmt_ptr()?;

    for i in 0..column_count {
      // name: The name of the column in the result set
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      let name = unsafe {
        let name_ptr = ffi::sqlite3_column_name(raw, i);
        if !name_ptr.is_null() {
          let name_cstr = std::ffi::CStr::from_ptr(name_ptr as _);
          v8::String::new_from_utf8(
            scope,
            name_cstr.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        } else {
          v8::null(scope).into()
        }
      };

      // column: The unaliased name of the column in the origin table
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      let column = unsafe {
        let column_ptr = ffi::sqlite3_column_origin_name(raw, i);
        if !column_ptr.is_null() {
          let column_cstr = std::ffi::CStr::from_ptr(column_ptr as _);
          v8::String::new_from_utf8(
            scope,
            column_cstr.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        } else {
          v8::null(scope).into()
        }
      };

      // table: The unaliased name of the origin table
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      let table = unsafe {
        let table_ptr = ffi::sqlite3_column_table_name(raw, i);
        if !table_ptr.is_null() {
          let table_cstr = std::ffi::CStr::from_ptr(table_ptr as _);
          v8::String::new_from_utf8(
            scope,
            table_cstr.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        } else {
          v8::null(scope).into()
        }
      };

      // database: The unaliased name of the origin database
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      let database = unsafe {
        let database_ptr = ffi::sqlite3_column_database_name(raw, i);
        if !database_ptr.is_null() {
          let database_cstr = std::ffi::CStr::from_ptr(database_ptr as _);
          v8::String::new_from_utf8(
            scope,
            database_cstr.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        } else {
          v8::null(scope).into()
        }
      };

      // type: The declared data type of the column
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt
      let col_type = unsafe {
        let type_ptr = ffi::sqlite3_column_decltype(raw, i);
        if !type_ptr.is_null() {
          let type_cstr = std::ffi::CStr::from_ptr(type_ptr as _);
          v8::String::new_from_utf8(
            scope,
            type_cstr.to_bytes(),
            v8::NewStringType::Normal,
          )
          .unwrap()
          .into()
        } else {
          v8::null(scope).into()
        }
      };

      let values = &[name, column, table, database, col_type];
      let null = v8::null(scope).into();
      let obj =
        v8::Object::with_prototype_and_properties(scope, null, keys, values);

      columns.push(obj.into());
    }

    Ok(v8::Array::new_with_elements(scope, &columns))
  }
}
