// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::c_char;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr::null;
use std::rc::Rc;

use deno_core::convert::OptionUndefined;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_permissions::PermissionsContainer;
use rusqlite::ffi as libsqlite3_sys;
use rusqlite::ffi::SQLITE_DBCONFIG_DQS_DDL;
use rusqlite::ffi::SQLITE_DBCONFIG_DQS_DML;
use rusqlite::limits::Limit;

use super::session::SessionOptions;
use super::statement::check_error_code2;
use super::validators;
use super::Session;
use super::SqliteError;
use super::StatementSync;

const SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION: i32 = 1005;
const SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE: i32 = 1021;

struct DatabaseSyncOptions {
  open: bool,
  enable_foreign_key_constraints: bool,
  read_only: bool,
  allow_extension: bool,
  enable_double_quoted_string_literals: bool,
}

impl<'a> FromV8<'a> for DatabaseSyncOptions {
  type Error = validators::Error;

  fn from_v8(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    use validators::Error;

    if value.is_undefined() {
      return Ok(Self::default());
    }

    let Ok(obj) = v8::Local::<v8::Object>::try_from(value) else {
      return Err(Error::InvalidArgType(
        "The \"options\" argument must be an object.",
      ));
    };

    let mut options = Self::default();

    v8_static_strings! {
      OPEN_STRING = "open",
      ENABLE_FOREIGN_KEY_CONSTRAINTS_STRING = "enableForeignKeyConstraints",
      READ_ONLY_STRING = "readOnly",
      ALLOW_EXTENSION_STRING = "allowExtension",
      ENABLE_DOUBLE_QUOTED_STRING_LITERALS_STRING = "enableDoubleQuotedStringLiterals",
    }

    let open_string = OPEN_STRING.v8_string(scope).unwrap();
    if let Some(open) = obj.get(scope, open_string.into()) {
      if !open.is_undefined() {
        options.open = v8::Local::<v8::Boolean>::try_from(open)
          .map_err(|_| {
            Error::InvalidArgType(
              "The \"options.open\" argument must be a boolean.",
            )
          })?
          .is_true();
      }
    }

    let read_only_string = READ_ONLY_STRING.v8_string(scope).unwrap();
    if let Some(read_only) = obj.get(scope, read_only_string.into()) {
      if !read_only.is_undefined() {
        options.read_only = v8::Local::<v8::Boolean>::try_from(read_only)
          .map_err(|_| {
            Error::InvalidArgType(
              "The \"options.readOnly\" argument must be a boolean.",
            )
          })?
          .is_true();
      }
    }

    let enable_foreign_key_constraints_string =
      ENABLE_FOREIGN_KEY_CONSTRAINTS_STRING
        .v8_string(scope)
        .unwrap();
    if let Some(enable_foreign_key_constraints) =
      obj.get(scope, enable_foreign_key_constraints_string.into())
    {
      if !enable_foreign_key_constraints.is_undefined() {
        options.enable_foreign_key_constraints =
          v8::Local::<v8::Boolean>::try_from(enable_foreign_key_constraints)
            .map_err(|_| {
              Error::InvalidArgType(
              "The \"options.enableForeignKeyConstraints\" argument must be a boolean.",
            )
            })?
            .is_true();
      }
    }

    let allow_extension_string =
      ALLOW_EXTENSION_STRING.v8_string(scope).unwrap();
    if let Some(allow_extension) = obj.get(scope, allow_extension_string.into())
    {
      if !allow_extension.is_undefined() {
        options.allow_extension =
          v8::Local::<v8::Boolean>::try_from(allow_extension)
            .map_err(|_| {
              Error::InvalidArgType(
                "The \"options.allowExtension\" argument must be a boolean.",
              )
            })?
            .is_true();
      }
    }

    let enable_double_quoted_string_literals_string =
      ENABLE_DOUBLE_QUOTED_STRING_LITERALS_STRING
        .v8_string(scope)
        .unwrap();
    if let Some(enable_double_quoted_string_literals) =
      obj.get(scope, enable_double_quoted_string_literals_string.into())
    {
      if !enable_double_quoted_string_literals.is_undefined() {
        options.enable_double_quoted_string_literals =
            v8::Local::<v8::Boolean>::try_from(enable_double_quoted_string_literals)
                .map_err(|_| {
                Error::InvalidArgType(
                    "The \"options.enableDoubleQuotedStringLiterals\" argument must be a boolean.",
                )
                })?
                .is_true();
      }
    }

    Ok(options)
  }
}

impl Default for DatabaseSyncOptions {
  fn default() -> Self {
    DatabaseSyncOptions {
      open: true,
      enable_foreign_key_constraints: true,
      read_only: false,
      allow_extension: false,
      enable_double_quoted_string_literals: false,
    }
  }
}

struct ApplyChangesetOptions<'a> {
  filter: Option<v8::Local<'a, v8::Value>>,
  on_conflict: Option<v8::Local<'a, v8::Value>>,
}

// Note: Can't use `FromV8` here because of lifetime issues with holding
// Local references.
impl<'a> ApplyChangesetOptions<'a> {
  fn from_value(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Option<Self>, validators::Error> {
    use validators::Error;

    if value.is_undefined() {
      return Ok(None);
    }

    let obj = v8::Local::<v8::Object>::try_from(value).map_err(|_| {
      Error::InvalidArgType("The \"options\" argument must be an object.")
    })?;

    let mut options = Self {
      filter: None,
      on_conflict: None,
    };

    v8_static_strings! {
      FILTER_STRING = "filter",
      ON_CONFLICT_STRING = "onConflict",
    }

    let filter_string = FILTER_STRING.v8_string(scope).unwrap();
    if let Some(filter) = obj.get(scope, filter_string.into()) {
      if !filter.is_undefined() {
        if !filter.is_function() {
          return Err(Error::InvalidArgType(
            "The \"options.filter\" argument must be a function.",
          ));
        }

        options.filter = Some(filter);
      }
    }

    let on_conflict_string = ON_CONFLICT_STRING.v8_string(scope).unwrap();
    if let Some(on_conflict) = obj.get(scope, on_conflict_string.into()) {
      if !on_conflict.is_undefined() {
        if !on_conflict.is_function() {
          return Err(Error::InvalidArgType(
            "The \"options.onConflict\" argument must be a function.",
          ));
        }

        options.on_conflict = Some(on_conflict);
      }
    }

    Ok(Some(options))
  }
}

pub struct DatabaseSync {
  conn: Rc<RefCell<Option<rusqlite::Connection>>>,
  statements: Rc<RefCell<Vec<*mut libsqlite3_sys::sqlite3_stmt>>>,
  options: DatabaseSyncOptions,
  location: String,
}

impl GarbageCollected for DatabaseSync {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"DatabaseSync"
  }
}

fn set_db_config(
  conn: &rusqlite::Connection,
  config: i32,
  value: bool,
) -> bool {
  // SAFETY: call to sqlite3_db_config is safe because the connection
  // handle is valid and the parameters are correct.
  unsafe {
    let mut set = 0;
    let r = libsqlite3_sys::sqlite3_db_config(
      conn.handle(),
      config,
      value as i32,
      &mut set,
    );

    if r != libsqlite3_sys::SQLITE_OK {
      panic!("Failed to set db config");
    }

    set == value as i32
  }
}

fn open_db(
  state: &mut OpState,
  readonly: bool,
  location: &str,
  allow_extension: bool,
) -> Result<rusqlite::Connection, SqliteError> {
  let perms = state.borrow::<PermissionsContainer>();
  if location == ":memory:" {
    let conn = rusqlite::Connection::open_in_memory()?;
    assert!(set_db_config(
      &conn,
      SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE,
      false
    ));

    if allow_extension {
      perms.check_ffi_all()?;
    } else {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
        false
      ));
    }

    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    return Ok(conn);
  }

  perms.check_read_with_api_name(location, Some("node:sqlite"))?;

  if readonly {
    let conn = rusqlite::Connection::open_with_flags(
      location,
      rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    assert!(set_db_config(
      &conn,
      SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE,
      false
    ));

    if allow_extension {
      perms.check_ffi_all()?;
    } else {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
        false
      ));
    }

    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    return Ok(conn);
  }

  perms.check_write_with_api_name(location, Some("node:sqlite"))?;

  let conn = rusqlite::Connection::open(location)?;

  if allow_extension {
    perms.check_ffi_all()?;
  } else {
    assert!(set_db_config(
      &conn,
      SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
      false
    ));
  }

  conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;

  Ok(conn)
}

fn database_constructor(
  _: &mut v8::HandleScope,
  args: &v8::FunctionCallbackArguments,
) -> Result<(), validators::Error> {
  // TODO(littledivy): use `IsConstructCall()`
  if args.new_target().is_undefined() {
    return Err(validators::Error::ConstructCallRequired);
  }

  Ok(())
}

fn is_open(
  scope: &mut v8::HandleScope,
  args: &v8::FunctionCallbackArguments,
) -> Result<(), SqliteError> {
  let this_ = args.this();
  let db = cppgc::try_unwrap_cppgc_object::<DatabaseSync>(scope, this_.into())
    .ok_or(SqliteError::AlreadyClosed)?;

  db.conn
    .borrow()
    .as_ref()
    .ok_or(SqliteError::AlreadyClosed)?;

  Ok(())
}

// Represents a single connection to a SQLite database.
#[op2]
impl DatabaseSync {
  // Constructs a new `DatabaseSync` instance.
  //
  // A SQLite database can be stored in a file or in memory. To
  // use a file-backed database, the `location` should be a path.
  // To use an in-memory database, the `location` should be special
  // name ":memory:".
  #[constructor]
  #[validate(database_constructor)]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[validate(validators::path_str)]
    #[string]
    location: String,
    #[from_v8] options: DatabaseSyncOptions,
  ) -> Result<DatabaseSync, SqliteError> {
    let db = if options.open {
      let db =
        open_db(state, options.read_only, &location, options.allow_extension)?;

      if options.enable_foreign_key_constraints {
        db.execute("PRAGMA foreign_keys = ON", [])?;
      } else {
        db.execute("PRAGMA foreign_keys = OFF", [])?;
      }

      set_db_config(
        &db,
        SQLITE_DBCONFIG_DQS_DDL,
        options.enable_double_quoted_string_literals,
      );
      set_db_config(
        &db,
        SQLITE_DBCONFIG_DQS_DML,
        options.enable_double_quoted_string_literals,
      );
      Some(db)
    } else {
      None
    };

    Ok(DatabaseSync {
      conn: Rc::new(RefCell::new(db)),
      statements: Rc::new(RefCell::new(Vec::new())),
      location,
      options,
    })
  }

  // Opens the database specified by `location` of this instance.
  //
  // This method should only be used when the database is not opened
  // via the constructor. An exception is thrown if the database is
  // already opened.
  #[fast]
  #[undefined]
  fn open(&self, state: &mut OpState) -> Result<(), SqliteError> {
    if self.conn.borrow().is_some() {
      return Err(SqliteError::AlreadyOpen);
    }

    let db = open_db(
      state,
      self.options.read_only,
      &self.location,
      self.options.allow_extension,
    )?;
    if self.options.enable_foreign_key_constraints {
      db.execute("PRAGMA foreign_keys = ON", [])?;
    } else {
      db.execute("PRAGMA foreign_keys = OFF", [])?;
    }

    set_db_config(
      &db,
      SQLITE_DBCONFIG_DQS_DDL,
      self.options.enable_double_quoted_string_literals,
    );
    set_db_config(
      &db,
      SQLITE_DBCONFIG_DQS_DML,
      self.options.enable_double_quoted_string_literals,
    );

    *self.conn.borrow_mut() = Some(db);

    Ok(())
  }

  // Closes the database connection. An exception is thrown if the
  // database is not open.
  #[fast]
  #[undefined]
  fn close(&self) -> Result<(), SqliteError> {
    if self.conn.borrow().is_none() {
      return Err(SqliteError::AlreadyClosed);
    }

    // Finalize all prepared statements
    for stmt in self.statements.borrow_mut().drain(..) {
      if !stmt.is_null() {
        // SAFETY: `stmt` is a valid statement handle.
        unsafe {
          libsqlite3_sys::sqlite3_finalize(stmt);
        }
      }
    }

    let _ = self.conn.borrow_mut().take();

    Ok(())
  }

  // This method allows one or more SQL statements to be executed
  // without returning any results.
  //
  // This method is a wrapper around sqlite3_exec().
  #[fast]
  #[validate(is_open)]
  #[undefined]
  fn exec(
    &self,
    #[validate(validators::sql_str)]
    #[string]
    sql: &str,
  ) -> Result<(), SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    db.execute_batch(sql)?;

    Ok(())
  }

  // Compiles an SQL statement into a prepared statement.
  //
  // This method is a wrapper around `sqlite3_prepare_v2()`.
  #[validate(is_open)]
  #[cppgc]
  fn prepare(
    &self,
    #[validate(validators::sql_str)]
    #[string]
    sql: &str,
  ) -> Result<StatementSync, SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    // SAFETY: lifetime of the connection is guaranteed by reference
    // counting.
    let raw_handle = unsafe { db.handle() };

    let mut raw_stmt = std::ptr::null_mut();

    // SAFETY: `sql` points to a valid memory location and its length
    // is correct.
    let r = unsafe {
      libsqlite3_sys::sqlite3_prepare_v2(
        raw_handle,
        sql.as_ptr() as *const _,
        sql.len() as i32,
        &mut raw_stmt,
        std::ptr::null_mut(),
      )
    };

    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::PrepareFailed);
    }

    self.statements.borrow_mut().push(raw_stmt);

    Ok(StatementSync {
      inner: raw_stmt,
      db: Rc::downgrade(&self.conn),
      statements: Rc::clone(&self.statements),
      use_big_ints: Cell::new(false),
      allow_bare_named_params: Cell::new(true),
      is_iter_finished: false,
    })
  }

  // Applies a changeset to the database.
  //
  // This method is a wrapper around `sqlite3changeset_apply()`.
  #[fast]
  #[reentrant]
  fn apply_changeset<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[validate(validators::changeset_buffer)]
    #[buffer]
    changeset: &[u8],
    options: v8::Local<'a, v8::Value>,
  ) -> Result<bool, SqliteError> {
    let options = ApplyChangesetOptions::from_value(scope, options)?;

    struct HandlerCtx<'a, 'b> {
      scope: &'a mut v8::HandleScope<'b>,
      confict: Option<v8::Local<'b, v8::Function>>,
      filter: Option<v8::Local<'b, v8::Function>>,
    }

    // Conflict handler callback for `sqlite3changeset_apply()`.
    unsafe extern "C" fn conflict_handler(
      p_ctx: *mut c_void,
      e_conflict: i32,
      _: *mut libsqlite3_sys::sqlite3_changeset_iter,
    ) -> i32 {
      let ctx = &mut *(p_ctx as *mut HandlerCtx);

      if let Some(conflict) = &mut ctx.confict {
        let recv = v8::undefined(ctx.scope).into();
        let args = [v8::Integer::new(ctx.scope, e_conflict).into()];

        let tc_scope = &mut v8::TryCatch::new(ctx.scope);

        let ret = conflict
          .call(tc_scope, recv, &args)
          .unwrap_or_else(|| v8::undefined(tc_scope).into());
        if tc_scope.has_caught() {
          tc_scope.rethrow();
          return libsqlite3_sys::SQLITE_CHANGESET_ABORT;
        }

        const INVALID_VALUE: i32 = -1;
        if !ret.is_int32() {
          return INVALID_VALUE;
        }

        let value = ret
          .int32_value(tc_scope)
          .unwrap_or(libsqlite3_sys::SQLITE_CHANGESET_ABORT);

        return value;
      }

      libsqlite3_sys::SQLITE_CHANGESET_ABORT
    }

    // Filter handler callback for `sqlite3changeset_apply()`.
    unsafe extern "C" fn filter_handler(
      p_ctx: *mut c_void,
      z_tab: *const c_char,
    ) -> i32 {
      let ctx = &mut *(p_ctx as *mut HandlerCtx);

      if let Some(filter) = &mut ctx.filter {
        let tab = CStr::from_ptr(z_tab).to_str().unwrap();

        let recv = v8::undefined(ctx.scope).into();
        let args = [v8::String::new(ctx.scope, tab).unwrap().into()];

        let ret = filter.call(ctx.scope, recv, &args).unwrap();
        return ret.boolean_value(ctx.scope) as i32;
      }

      1
    }

    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    // It is safe to use scope in the handlers because they are never
    // called after the call to `sqlite3changeset_apply()`.
    let mut ctx = HandlerCtx {
      scope,
      confict: None,
      filter: None,
    };

    if let Some(options) = options {
      if let Some(filter) = options.filter {
        let filter_cb: v8::Local<v8::Function> = filter
          .try_into()
          .map_err(|_| SqliteError::InvalidCallback("filter"))?;
        ctx.filter = Some(filter_cb);
      }

      if let Some(on_conflict) = options.on_conflict {
        let on_conflict_cb: v8::Local<v8::Function> = on_conflict
          .try_into()
          .map_err(|_| SqliteError::InvalidCallback("onConflict"))?;
        ctx.confict = Some(on_conflict_cb);
      }
    }

    // SAFETY: lifetime of the connection is guaranteed by reference
    // counting.
    let raw_handle = unsafe { db.handle() };

    // SAFETY: `changeset` points to a valid memory location and its
    // length is correct. `ctx` is stack allocated and its lifetime is
    // longer than the call to `sqlite3changeset_apply()`.
    unsafe {
      let r = libsqlite3_sys::sqlite3changeset_apply(
        raw_handle,
        changeset.len() as i32,
        changeset.as_ptr() as *mut _,
        Some(filter_handler),
        Some(conflict_handler),
        &mut ctx as *mut _ as *mut c_void,
      );

      if r == libsqlite3_sys::SQLITE_OK {
        return Ok(true);
      } else if r == libsqlite3_sys::SQLITE_ABORT {
        return Ok(false);
      }

      check_error_code2(r)?;

      Ok(false)
    }
  }

  // Loads a SQLite extension.
  //
  // This is a wrapper around `sqlite3_load_extension`. It requires FFI permission
  // to be granted and allowExtension must be set to true when opening the database.
  fn load_extension(
    &self,
    state: &mut OpState,
    #[validate(validators::path_str)]
    #[string]
    path: &str,
    #[string] entry_point: Option<String>,
  ) -> Result<(), SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    if !self.options.allow_extension {
      return Err(SqliteError::LoadExensionFailed(
        "Cannot load SQLite extensions when allowExtension is not enabled"
          .to_string(),
      ));
    }

    state.borrow::<PermissionsContainer>().check_ffi_all()?;

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let raw_handle = unsafe { db.handle() };

    let path_cstring = std::ffi::CString::new(path.as_bytes())?;
    let entry_point_cstring =
      entry_point.map(|ep| std::ffi::CString::new(ep).unwrap_or_default());

    let entry_point_ptr = match &entry_point_cstring {
      Some(cstr) => cstr.as_ptr(),
      None => std::ptr::null(),
    };

    let mut err_msg: *mut c_char = std::ptr::null_mut();

    // SAFETY: Using sqlite3_load_extension with proper error handling
    let result = unsafe {
      let res = libsqlite3_sys::sqlite3_load_extension(
        raw_handle,
        path_cstring.as_ptr(),
        entry_point_ptr,
        &mut err_msg,
      );

      if res != libsqlite3_sys::SQLITE_OK {
        let error_message = if !err_msg.is_null() {
          let c_str = std::ffi::CStr::from_ptr(err_msg);
          let message = c_str.to_string_lossy().into_owned();
          libsqlite3_sys::sqlite3_free(err_msg as *mut _);
          message
        } else {
          format!("Failed to load extension with error code: {}", res)
        };

        return Err(SqliteError::LoadExensionFailed(error_message));
      }

      res
    };

    if result == libsqlite3_sys::SQLITE_OK {
      Ok(())
    } else {
      Err(SqliteError::LoadExensionFailed(
        "Unknown error loading SQLite extension".to_string(),
      ))
    }
  }

  // Creates and attaches a session to the database.
  //
  // This method is a wrapper around `sqlite3session_create()` and
  // `sqlite3session_attach()`.
  #[cppgc]
  fn create_session(
    &self,
    #[from_v8] options: OptionUndefined<SessionOptions>,
  ) -> Result<Session, SqliteError> {
    let options = options.0;
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    // SAFETY: lifetime of the connection is guaranteed by reference
    // counting.
    let raw_handle = unsafe { db.handle() };

    let mut raw_session = std::ptr::null_mut();
    let mut options = options;

    let z_db = options
      .as_mut()
      .and_then(|options| options.db.take())
      .map(|db| CString::new(db).unwrap())
      .unwrap_or_else(|| CString::new("main").unwrap());
    // SAFETY: `z_db` points to a valid c-string.
    let r = unsafe {
      libsqlite3_sys::sqlite3session_create(
        raw_handle,
        z_db.as_ptr() as *const _,
        &mut raw_session,
      )
    };

    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::SessionCreateFailed);
    }

    let table = options
      .as_mut()
      .and_then(|options| options.table.take())
      .map(|table| CString::new(table).unwrap());
    let z_table = table.as_ref().map(|table| table.as_ptr()).unwrap_or(null());
    let r =
      // SAFETY: `z_table` points to a valid c-string and `raw_session`
      // is a valid session handle.
      unsafe { libsqlite3_sys::sqlite3session_attach(raw_session, z_table) };

    if r != libsqlite3_sys::SQLITE_OK {
      return Err(SqliteError::SessionCreateFailed);
    }

    Ok(Session {
      inner: raw_session,
      freed: Cell::new(false),
      db: Rc::downgrade(&self.conn),
    })
  }
}
