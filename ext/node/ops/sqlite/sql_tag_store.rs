// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use rusqlite::ffi as libsqlite3_sys;

use super::SqliteError;
use super::lru_cache::LRUCache;
use super::statement::InnerStatementPtr;
use super::statement::check_error_code;
use crate::ops::sqlite::statement::statement_bind_value;
use crate::ops::sqlite::statement::statement_column_value;

struct CachedStatement {
  inner: InnerStatementPtr,
  return_arrays: bool,
  use_big_ints: bool,
}

impl CachedStatement {
  fn is_finalized(&self) -> bool {
    self.inner.get().is_none()
  }

  fn stmt_ptr(&self) -> Result<*mut libsqlite3_sys::sqlite3_stmt, SqliteError> {
    let ptr = self.inner.get();
    match ptr {
      Some(p) => Ok(p),
      None => Err(SqliteError::StatementFinalized),
    }
  }

  fn reset(&self) -> Result<(), SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
    let r = unsafe { libsqlite3_sys::sqlite3_reset(raw) };
    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::StatementFinalized);
    }
    Ok(())
  }

  fn step(&self) -> Result<bool, SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
    unsafe {
      let r = libsqlite3_sys::sqlite3_step(raw);
      if r == libsqlite3_sys::SQLITE_DONE {
        return Ok(true);
      }
      if r != libsqlite3_sys::SQLITE_ROW {
        return Err(SqliteError::StatementFinalized);
      }
    }
    Ok(false)
  }

  fn column_count(&self) -> Result<i32, SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
    let count = unsafe { libsqlite3_sys::sqlite3_column_count(raw) };
    Ok(count)
  }

  fn column_name(&self, index: i32) -> Result<&[u8], SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
    unsafe {
      let name = libsqlite3_sys::sqlite3_column_name(raw, index);
      Ok(std::ffi::CStr::from_ptr(name as _).to_bytes())
    }
  }

  fn column_value<'a>(
    &self,
    index: i32,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    statement_column_value(self.stmt_ptr()?, index, self.use_big_ints, scope)
  }

  fn bind_value(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<v8::Value>,
    index: i32,
  ) -> Result<(), SqliteError> {
    let r = statement_bind_value(self.stmt_ptr()?, index, value, scope)?;

    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::FailedBind("Failed to bind value"));
    }
    Ok(())
  }

  fn clear_bindings(&self) -> Result<(), SqliteError> {
    let raw = self.stmt_ptr()?;
    // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
    let r = unsafe { libsqlite3_sys::sqlite3_clear_bindings(raw) };
    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::FailedBind("Failed to clear bindings"));
    }
    Ok(())
  }

  fn bind_params(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    args: &v8::FunctionCallbackArguments,
    start_index: usize,
  ) -> Result<(), SqliteError> {
    self.reset()?;
    self.clear_bindings()?;

    let raw = self.stmt_ptr()?;
    let param_count =
      // SAFETY: `raw` is a valid pointer to a sqlite3_stmt.
      unsafe { libsqlite3_sys::sqlite3_bind_parameter_count(raw) };
    let n_params = args.length() as usize - start_index;

    for i in 0..n_params.min(param_count as usize) {
      let value = args.get((start_index + i) as i32);
      self.bind_value(scope, value, (i + 1) as i32)?;
    }

    Ok(())
  }

  fn read_row<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<Option<v8::Local<'a, v8::Value>>, SqliteError> {
    if self.step()? {
      return Ok(None);
    }

    let num_cols = self.column_count()?;
    let mut names = Vec::with_capacity(num_cols as usize);
    let mut values = Vec::with_capacity(num_cols as usize);

    for i in 0..num_cols {
      let name = self.column_name(i)?;
      let value = self.column_value(i, scope)?;
      let name =
        v8::String::new_from_utf8(scope, name, v8::NewStringType::Normal)
          .unwrap()
          .into();
      names.push(name);
      values.push(value);
    }

    if self.return_arrays {
      let result = v8::Array::new_with_elements(scope, &values);
      Ok(Some(result.into()))
    } else {
      let null = v8::null(scope).into();
      let result =
        v8::Object::with_prototype_and_properties(scope, null, &names, &values);
      Ok(Some(result.into()))
    }
  }
}

pub struct SQLTagStore {
  db: Rc<RefCell<Option<rusqlite::Connection>>>,
  statements: Rc<RefCell<Vec<InnerStatementPtr>>>,
  cache: RefCell<LRUCache<String, CachedStatement>>,
  capacity: u32,
  return_arrays: bool,
  use_big_ints: bool,
  db_object: v8::Global<v8::Object>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for SQLTagStore {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"SQLTagStore"
  }
}

impl SQLTagStore {
  pub fn create(
    db: Rc<RefCell<Option<rusqlite::Connection>>>,
    statements: Rc<RefCell<Vec<InnerStatementPtr>>>,
    capacity: u32,
    return_arrays: bool,
    use_big_ints: bool,
    db_object: v8::Global<v8::Object>,
  ) -> Self {
    SQLTagStore {
      db,
      statements,
      cache: RefCell::new(LRUCache::new(capacity as usize)),
      capacity,
      return_arrays,
      use_big_ints,
      db_object,
    }
  }

  // Parse template literal strings and interpolated values to build
  // SQL query with ? placeholders.
  fn parse_template<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    args: &v8::FunctionCallbackArguments,
  ) -> Result<String, SqliteError> {
    if args.length() < 1 {
      return Err(SqliteError::Validation(
        super::validators::Error::InvalidArgType(
          "First argument must be an array of strings (template literal).",
        ),
      ));
    }

    let first = args.get(0);
    if !first.is_array() {
      return Err(SqliteError::Validation(
        super::validators::Error::InvalidArgType(
          "First argument must be an array of strings (template literal).",
        ),
      ));
    }

    let strings: v8::Local<v8::Array> = first.try_into().unwrap();
    let n_strings = strings.length();
    let n_params = (args.length() - 1) as u32;

    let mut sql = String::new();
    for i in 0..n_strings {
      let str_val = strings.get_index(scope, i).unwrap();
      if !str_val.is_string() {
        return Err(SqliteError::Validation(
          super::validators::Error::InvalidArgType(
            "Template literal parts must be strings.",
          ),
        ));
      }
      sql.push_str(&str_val.to_rust_string_lossy(scope));
      if i < n_params {
        sql.push('?');
      }
    }

    Ok(sql)
  }

  // Prepare or retrieve a cached statement.
  fn prepare_statement<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    args: &v8::FunctionCallbackArguments,
  ) -> Result<std::cell::RefMut<'_, CachedStatement>, SqliteError> {
    let db = self.db.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    let sql = Self::parse_template(scope, args)?;

    {
      let mut cache = self.cache.borrow_mut();
      if let Some(stmt) = cache.get_mut(&sql) {
        if !stmt.is_finalized() {
          // Update settings from store
          stmt.return_arrays = self.return_arrays;
          stmt.use_big_ints = self.use_big_ints;
          // Need to return, but can't borrow mut twice
          drop(cache);
          return Ok(self.get_cached_statement(&sql));
        }
        // Statement is finalized, remove it
        cache.erase(&sql);
      }
    }

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let raw_handle = unsafe { db.handle() };

    let mut raw_stmt = std::ptr::null_mut();

    // SAFETY: `sql` points to valid memory.
    let r = unsafe {
      libsqlite3_sys::sqlite3_prepare_v2(
        raw_handle,
        sql.as_ptr() as *const _,
        sql.len() as i32,
        &mut raw_stmt,
        std::ptr::null_mut(),
      )
    };
    check_error_code(r, raw_handle)?;

    let stmt_cell = Rc::new(Cell::new(Some(raw_stmt)));
    self.statements.borrow_mut().push(stmt_cell.clone());

    let cached_stmt = CachedStatement {
      inner: stmt_cell,
      return_arrays: self.return_arrays,
      use_big_ints: self.use_big_ints,
    };

    self.cache.borrow_mut().put(sql.clone(), cached_stmt);
    Ok(self.get_cached_statement(&sql))
  }

  fn get_cached_statement(
    &self,
    sql: &str,
  ) -> std::cell::RefMut<'_, CachedStatement> {
    let cache = self.cache.borrow_mut();
    std::cell::RefMut::map(cache, |c| c.get_mut(&sql.to_string()).unwrap())
  }
}

struct ResetGuard<'a>(&'a CachedStatement);

impl Drop for ResetGuard<'_> {
  fn drop(&mut self) {
    let _ = self.0.reset();
  }
}

#[op2]
impl SQLTagStore {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> Result<SQLTagStore, SqliteError> {
    Err(SqliteError::InvalidConstructor)
  }

  #[reentrant]
  fn get<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    let args = args.ok_or(SqliteError::Validation(
      super::validators::Error::InvalidArgType(
        "First argument must be an array of strings (template literal).",
      ),
    ))?;

    let stmt = self.prepare_statement(scope, args)?;
    stmt.bind_params(scope, args, 1)?;

    let _reset = ResetGuard(&stmt);
    let entry = stmt.read_row(scope)?;
    let result = entry.unwrap_or_else(|| v8::undefined(scope).into());

    Ok(result)
  }

  fn run<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    let args = args.ok_or(SqliteError::Validation(
      super::validators::Error::InvalidArgType(
        "First argument must be an array of strings (template literal).",
      ),
    ))?;

    let stmt = self.prepare_statement(scope, args)?;
    stmt.bind_params(scope, args, 1)?;

    let _reset = ResetGuard(&stmt);
    stmt.step()?;
    drop(_reset);

    let db = self.db.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    v8_static_strings! {
      LAST_INSERT_ROW_ID = "lastInsertRowid",
      CHANGES = "changes",
    }

    let obj = v8::Object::new(scope);

    let last_insert_row_id_str = LAST_INSERT_ROW_ID.v8_string(scope).unwrap();
    let last_insert_rowid = db.last_insert_rowid();
    let last_insert_row_id_val = if self.use_big_ints {
      v8::BigInt::new_from_i64(scope, last_insert_rowid).into()
    } else {
      v8::Number::new(scope, last_insert_rowid as f64).into()
    };

    obj
      .set(scope, last_insert_row_id_str.into(), last_insert_row_id_val)
      .unwrap();

    let changes_str = CHANGES.v8_string(scope).unwrap();
    let changes = db.changes();
    let changes_val = if self.use_big_ints {
      v8::BigInt::new_from_u64(scope, changes).into()
    } else {
      v8::Number::new(scope, changes as f64).into()
    };

    obj.set(scope, changes_str.into(), changes_val).unwrap();

    Ok(obj.into())
  }

  fn all<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Array>, SqliteError> {
    let args = args.ok_or(SqliteError::Validation(
      super::validators::Error::InvalidArgType(
        "First argument must be an array of strings (template literal).",
      ),
    ))?;

    let stmt = self.prepare_statement(scope, args)?;
    stmt.bind_params(scope, args, 1)?;

    let _reset = ResetGuard(&stmt);
    let mut arr = vec![];
    while let Some(result) = stmt.read_row(scope)? {
      arr.push(result);
    }

    let arr = v8::Array::new_with_elements(scope, &arr);
    Ok(arr)
  }

  fn iterate<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Object>, SqliteError> {
    v8_static_strings! {
      ITERATOR = "Iterator",
      PROTOTYPE = "prototype",
      NEXT = "next",
      RETURN = "return",
      DONE = "done",
      VALUE = "value",
    }

    let args = args.ok_or(SqliteError::Validation(
      super::validators::Error::InvalidArgType(
        "First argument must be an array of strings (template literal).",
      ),
    ))?;

    let db = self.db.borrow();
    let _ = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    let sql = Self::parse_template(scope, args)?;
    drop(db);

    {
      let mut cache = self.cache.borrow_mut();
      if let Some(stmt) = cache.get_mut(&sql) {
        if stmt.is_finalized() {
          cache.erase(&sql);
        }
      }
    }

    let db = self.db.borrow();
    let db_ref = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    if !self.cache.borrow().exists(&sql) {
      // SAFETY: lifetime of the connection is guaranteed by reference counting.
      let raw_handle = unsafe { db_ref.handle() };

      let mut raw_stmt = std::ptr::null_mut();

      // SAFETY: `sql` points to valid memory.
      let r = unsafe {
        libsqlite3_sys::sqlite3_prepare_v2(
          raw_handle,
          sql.as_ptr() as *const _,
          sql.len() as i32,
          &mut raw_stmt,
          std::ptr::null_mut(),
        )
      };
      check_error_code(r, raw_handle)?;

      let stmt_cell = Rc::new(Cell::new(Some(raw_stmt)));
      self.statements.borrow_mut().push(stmt_cell.clone());

      let cached_stmt = CachedStatement {
        inner: stmt_cell,
        return_arrays: self.return_arrays,
        use_big_ints: self.use_big_ints,
      };

      self.cache.borrow_mut().put(sql.clone(), cached_stmt);
    }
    drop(db);

    {
      let mut stmt = self.get_cached_statement(&sql);
      stmt.return_arrays = self.return_arrays;
      stmt.use_big_ints = self.use_big_ints;
      stmt.bind_params(scope, args, 1)?;
    }

    // Create an iterator object with next and return methods
    // Store context needed for iteration
    // TODO: Check if this struct may never be dropped
    struct IterContext {
      sql: String,
      done: Cell<bool>,
    }

    let iter_context = Box::new(IterContext {
      sql: sql.clone(),
      done: Cell::new(false),
    });

    let iterate_next = |scope: &mut v8::PinScope<'_, '_>,
                        fargs: v8::FunctionCallbackArguments,
                        mut rv: v8::ReturnValue| {
      let data = v8::Local::<v8::External>::try_from(fargs.data())
        .expect("Iterator#next expected external data");
      // SAFETY: `data` contains our iter context
      let (ctx, store) = unsafe {
        let pair = data.value() as *mut (IterContext, *const SQLTagStore);
        (&(*pair).0, &*(*pair).1)
      };

      let names = &[
        DONE.v8_string(scope).unwrap().into(),
        VALUE.v8_string(scope).unwrap().into(),
      ];

      if ctx.done.get() {
        let values =
          &[v8::Boolean::new(scope, true).into(), v8::null(scope).into()];
        let null = v8::null(scope).into();
        let result =
          v8::Object::with_prototype_and_properties(scope, null, names, values);
        rv.set(result.into());
        return;
      }

      let result = {
        let stmt = store.get_cached_statement(&ctx.sql);
        stmt.read_row(scope)
      };

      match result {
        Ok(Some(row)) => {
          let values = &[v8::Boolean::new(scope, false).into(), row];
          let null = v8::null(scope).into();
          let result = v8::Object::with_prototype_and_properties(
            scope, null, names, values,
          );
          rv.set(result.into());
        }
        Ok(None) | Err(_) => {
          ctx.done.set(true);
          let _ = {
            let stmt = store.get_cached_statement(&ctx.sql);
            stmt.reset()
          };
          let values =
            &[v8::Boolean::new(scope, true).into(), v8::null(scope).into()];
          let null = v8::null(scope).into();
          let result = v8::Object::with_prototype_and_properties(
            scope, null, names, values,
          );
          rv.set(result.into());
        }
      }
    };

    let iterate_return = |scope: &mut v8::PinScope<'_, '_>,
                          fargs: v8::FunctionCallbackArguments,
                          mut rv: v8::ReturnValue| {
      let data = v8::Local::<v8::External>::try_from(fargs.data())
        .expect("Iterator#return expected external data");
      // SAFETY: `data` contains our iter context
      let (ctx, store) = unsafe {
        let pair = data.value() as *mut (IterContext, *const SQLTagStore);
        (&(*pair).0, &*(*pair).1)
      };

      ctx.done.set(true);
      let _ = {
        let stmt = store.get_cached_statement(&ctx.sql);
        stmt.reset()
      };

      let names = &[
        DONE.v8_string(scope).unwrap().into(),
        VALUE.v8_string(scope).unwrap().into(),
      ];
      let values =
        &[v8::Boolean::new(scope, true).into(), v8::null(scope).into()];
      let null = v8::null(scope).into();
      let result =
        v8::Object::with_prototype_and_properties(scope, null, names, values);
      rv.set(result.into());
    };

    let combined = Box::new((*iter_context, self as *const _));
    let combined_ptr = Box::into_raw(combined);

    let external = v8::External::new(scope, combined_ptr as _);
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

    let values: &[v8::Local<v8::Value>] =
      &[next_func.into(), return_func.into()];
    let iterator = v8::Object::with_prototype_and_properties(
      scope,
      js_iterator_proto,
      names,
      values,
    );

    Ok(iterator)
  }

  #[fast]
  #[number]
  fn size(&self) -> u64 {
    self.cache.borrow().size() as u64
  }

  #[getter]
  #[number]
  fn capacity(&self) -> u64 {
    self.capacity as u64
  }

  #[getter]
  #[global]
  fn db(&self) -> v8::Global<v8::Object> {
    self.db_object.clone()
  }

  #[fast]
  #[undefined]
  fn clear(&self) {
    self.cache.borrow_mut().clear();
  }
}
