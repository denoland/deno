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
use super::statement::StatementExecution;
use super::statement::check_error_code;

struct CachedStatement {
  inner: InnerStatementPtr,
  return_arrays: bool,
  use_big_ints: bool,
}

impl CachedStatement {
  fn is_finalized(&self) -> bool {
    self.inner.get().is_none()
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
}

impl StatementExecution for CachedStatement {
  fn stmt_ptr(&self) -> Result<*mut libsqlite3_sys::sqlite3_stmt, SqliteError> {
    let ptr = self.inner.get();
    match ptr {
      Some(p) => Ok(p),
      None => Err(SqliteError::StatementFinalized),
    }
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

  fn return_arrays(&self) -> bool {
    self.return_arrays
  }

  fn use_big_ints(&self) -> bool {
    self.use_big_ints
  }

  fn check_bind_result(&self, r: i32) -> Result<(), SqliteError> {
    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::FailedBind("Failed to bind value"));
    }
    Ok(())
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
  iter_contexts: RefCell<Vec<*mut SQLTagStoreIteratorContext>>,
}

struct SQLTagStoreIteratorContext {
  store: *const SQLTagStore,
  store_ref: v8::Global<v8::Value>,
  sql: String,
  finished: Cell<bool>,
  finalized_functions: Cell<u8>,
  next_func: RefCell<Option<v8::Weak<v8::Function>>>,
  return_func: RefCell<Option<v8::Weak<v8::Function>>>,
}

impl Drop for SQLTagStore {
  fn drop(&mut self) {
    for ctx_ptr in self.iter_contexts.borrow().iter() {
      // SAFETY: Each pointer was allocated via `Box::into_raw` in `iterate()`
      // and has not been freed yet (freed contexts are removed from the vec).
      unsafe {
        drop(Box::from_raw(*ctx_ptr));
      }
    }
  }
}

fn release_iterator_context(ctx_ptr: *mut SQLTagStoreIteratorContext) {
  // SAFETY: `ctx_ptr` was allocated with `Box::into_raw` in `iterate()` and
  // remains valid until both callback functions have been finalized.
  let ctx = unsafe { &*ctx_ptr };
  let finalized_functions = ctx.finalized_functions.get() + 1;
  ctx.finalized_functions.set(finalized_functions);
  if finalized_functions != 2 {
    return;
  }

  // SAFETY: `ctx.store_ref` keeps the JS wrapper, and therefore this cppgc
  // object, alive until the context is dropped below.
  let store = unsafe { &*ctx.store };
  store
    .iter_contexts
    .borrow_mut()
    .retain(|ptr| *ptr != ctx_ptr);

  // SAFETY: Both callback functions are gone, so no `v8::External` can reach
  // this context again.
  unsafe {
    drop(Box::from_raw(ctx_ptr));
  }
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
      iter_contexts: RefCell::new(Vec::new()),
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
          "First argument must be an array of strings (template literal)."
            .into(),
        ),
      ));
    }

    let first = args.get(0);
    if !first.is_array() {
      return Err(SqliteError::Validation(
        super::validators::Error::InvalidArgType(
          "First argument must be an array of strings (template literal)."
            .into(),
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
            "Template literal parts must be strings.".into(),
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
        "First argument must be an array of strings (template literal).".into(),
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
        "First argument must be an array of strings (template literal).".into(),
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
        "First argument must be an array of strings (template literal).".into(),
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
    #[varargs] params: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<v8::Local<'a, v8::Object>, SqliteError> {
    v8_static_strings! {
      ITERATOR = "Iterator",
      PROTOTYPE = "prototype",
      NEXT = "next",
      RETURN = "return",
      DONE = "done",
      VALUE = "value",
      __STATEMENT_REF = "__statement_ref",
    }

    let args = params.ok_or(SqliteError::Validation(
      super::validators::Error::InvalidArgType(
        "First argument must be an array of strings (template literal).".into(),
      ),
    ))?;
    // `args.this()` is stored in the iterator context below and keeps the
    // SQLTagStore cppgc wrapper alive for detached `next`/`return` callbacks.

    {
      let db = self.db.borrow();
      let _ = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;
    }
    let sql = Self::parse_template(scope, args)?;

    {
      let mut cache = self.cache.borrow_mut();
      if let Some(stmt) = cache.get_mut(&sql)
        && stmt.is_finalized()
      {
        cache.erase(&sql);
      }
    }

    {
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
    }

    {
      let mut stmt = self.get_cached_statement(&sql);
      stmt.return_arrays = self.return_arrays;
      stmt.use_big_ints = self.use_big_ints;
      stmt.bind_params(scope, args, 1)?;
    }

    let store_ref = v8::Global::new(scope, args.this().cast::<v8::Value>());
    let iter_ctx = Box::into_raw(Box::new(SQLTagStoreIteratorContext {
      store: self as *const SQLTagStore,
      store_ref,
      sql,
      finished: Cell::new(false),
      finalized_functions: Cell::new(0),
      next_func: RefCell::new(None),
      return_func: RefCell::new(None),
    }));
    self.iter_contexts.borrow_mut().push(iter_ctx);

    let iterate_next = |scope: &mut v8::PinScope<'_, '_>,
                        fargs: v8::FunctionCallbackArguments,
                        mut rv: v8::ReturnValue| {
      let data = v8::Local::<v8::External>::try_from(fargs.data())
        .expect("Iterator#next expected external data");
      // SAFETY: `data` points to a live iterator context kept alive by the
      // callback functions' weak handles.
      let ctx =
        unsafe { &*(data.value() as *const SQLTagStoreIteratorContext) };
      // SAFETY: The context's strong JS wrapper reference keeps the store's
      // cppgc object alive.
      let store = unsafe { &*ctx.store };

      let names = &[
        DONE.v8_string(scope).unwrap().into(),
        VALUE.v8_string(scope).unwrap().into(),
      ];

      // If the cached statement was evicted mid-iteration, finish the iterator
      // instead of re-preparing and restarting from the first row.
      if ctx.finished.get() || !store.cache.borrow().exists(&ctx.sql) {
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
          ctx.finished.set(true);
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
      // SAFETY: `data` points to a live iterator context kept alive by the
      // callback functions' weak handles.
      let ctx =
        unsafe { &*(data.value() as *const SQLTagStoreIteratorContext) };
      // SAFETY: The context's strong JS wrapper reference keeps the store's
      // cppgc object alive.
      let store = unsafe { &*ctx.store };

      ctx.finished.set(true);
      if store.cache.borrow().exists(&ctx.sql) {
        let _ = {
          let stmt = store.get_cached_statement(&ctx.sql);
          stmt.reset()
        };
      }

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

    let external = v8::External::new(scope, iter_ctx as _);
    let next_func = v8::Function::builder(iterate_next)
      .data(external.into())
      .build(scope)
      .expect("Failed to create Iterator#next function");
    let return_func = v8::Function::builder(iterate_return)
      .data(external.into())
      .build(scope)
      .expect("Failed to create Iterator#return function");

    let weak_next = v8::Weak::with_finalizer(
      scope,
      next_func,
      Box::new(move |_| release_iterator_context(iter_ctx)),
    );
    let weak_return = v8::Weak::with_finalizer(
      scope,
      return_func,
      Box::new(move |_| release_iterator_context(iter_ctx)),
    );
    // SAFETY: `iter_ctx` was allocated above and is kept alive by the weak
    // handles stored inside it.
    unsafe {
      *(*iter_ctx).next_func.borrow_mut() = Some(weak_next);
      *(*iter_ctx).return_func.borrow_mut() = Some(weak_return);
    }

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
    // SAFETY: `iter_ctx` was allocated above and remains alive at least until
    // both generated callback functions are finalized.
    let store_ref = v8::Local::new(scope, unsafe { &(*iter_ctx).store_ref });
    let attrs = v8::PropertyAttribute::READ_ONLY
      | v8::PropertyAttribute::DONT_ENUM
      | v8::PropertyAttribute::DONT_DELETE;
    let store_ref_key = __STATEMENT_REF.v8_string(scope).unwrap().into();
    iterator
      .define_own_property(scope, store_ref_key, store_ref, attrs)
      .unwrap();

    Ok(iterator)
  }

  #[getter]
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
  fn db(&self) -> v8::Global<v8::Object> {
    self.db_object.clone()
  }

  #[fast]
  #[undefined]
  fn clear(&self) {
    self.cache.borrow_mut().clear();
  }
}
