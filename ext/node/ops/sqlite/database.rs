// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;
use std::ptr::null;
use std::rc::Rc;

use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::convert::OptionUndefined;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use rusqlite::ffi as libsqlite3_sys;
use rusqlite::ffi::SQLITE_DBCONFIG_DQS_DDL;
use rusqlite::ffi::SQLITE_DBCONFIG_DQS_DML;
use rusqlite::ffi::sqlite3_create_window_function;
use rusqlite::ffi::sqlite3_db_filename;
use rusqlite::limits::Limit;

use super::Session;
use super::SqliteError;
use super::StatementSync;
use super::session::SessionOptions;
use super::statement::InnerStatementPtr;
use super::statement::check_error_code;
use super::statement::check_error_code2;
use super::validators;

const SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION: i32 = 1005;
const SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE: i32 = 1021;
const MAX_SAFE_JS_INTEGER: i64 = 9_007_199_254_740_991;

struct DatabaseSyncOptions {
  open: bool,
  enable_foreign_key_constraints: bool,
  read_only: bool,
  allow_extension: bool,
  enable_double_quoted_string_literals: bool,
  timeout: u64,
}

impl<'a> FromV8<'a> for DatabaseSyncOptions {
  type Error = validators::Error;

  fn from_v8(
    scope: &mut v8::PinScope<'a, '_>,
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
      TIMEOUT_STRING = "timeout",
    }

    let open_string = OPEN_STRING.v8_string(scope).unwrap();
    if let Some(open) = obj.get(scope, open_string.into())
      && !open.is_undefined()
    {
      options.open = v8::Local::<v8::Boolean>::try_from(open)
        .map_err(|_| {
          Error::InvalidArgType(
            "The \"options.open\" argument must be a boolean.",
          )
        })?
        .is_true();
    }

    let read_only_string = READ_ONLY_STRING.v8_string(scope).unwrap();
    if let Some(read_only) = obj.get(scope, read_only_string.into())
      && !read_only.is_undefined()
    {
      options.read_only = v8::Local::<v8::Boolean>::try_from(read_only)
        .map_err(|_| {
          Error::InvalidArgType(
            "The \"options.readOnly\" argument must be a boolean.",
          )
        })?
        .is_true();
    }

    let enable_foreign_key_constraints_string =
      ENABLE_FOREIGN_KEY_CONSTRAINTS_STRING
        .v8_string(scope)
        .unwrap();
    if let Some(enable_foreign_key_constraints) =
      obj.get(scope, enable_foreign_key_constraints_string.into())
      && !enable_foreign_key_constraints.is_undefined()
    {
      options.enable_foreign_key_constraints =
          v8::Local::<v8::Boolean>::try_from(enable_foreign_key_constraints)
            .map_err(|_| {
              Error::InvalidArgType(
              "The \"options.enableForeignKeyConstraints\" argument must be a boolean.",
            )
            })?
            .is_true();
    }

    let allow_extension_string =
      ALLOW_EXTENSION_STRING.v8_string(scope).unwrap();
    if let Some(allow_extension) = obj.get(scope, allow_extension_string.into())
      && !allow_extension.is_undefined()
    {
      options.allow_extension =
        v8::Local::<v8::Boolean>::try_from(allow_extension)
          .map_err(|_| {
            Error::InvalidArgType(
              "The \"options.allowExtension\" argument must be a boolean.",
            )
          })?
          .is_true();
    }

    let enable_double_quoted_string_literals_string =
      ENABLE_DOUBLE_QUOTED_STRING_LITERALS_STRING
        .v8_string(scope)
        .unwrap();
    if let Some(enable_double_quoted_string_literals) =
      obj.get(scope, enable_double_quoted_string_literals_string.into())
      && !enable_double_quoted_string_literals.is_undefined()
    {
      options.enable_double_quoted_string_literals =
            v8::Local::<v8::Boolean>::try_from(enable_double_quoted_string_literals)
                .map_err(|_| {
                Error::InvalidArgType(
                    "The \"options.enableDoubleQuotedStringLiterals\" argument must be a boolean.",
                )
                })?
                .is_true();
    }

    let timeout_string = TIMEOUT_STRING.v8_string(scope).unwrap();
    if let Some(timeout) = obj.get(scope, timeout_string.into())
      && !timeout.is_undefined()
    {
      let timeout = v8::Local::<v8::Integer>::try_from(timeout)
        .map_err(|_| {
          Error::InvalidArgType(
            "The \"options.timeout\" argument must be an integer.",
          )
        })?
        .value();

      if timeout > 0 {
        options.timeout = timeout as u64;
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
      timeout: 0,
    }
  }
}

struct AggregateFunctionOption<'a> {
  deterministic: bool,
  direct_only: bool,
  use_big_int_arguments: bool,
  varargs: bool,
  start: v8::Local<'a, v8::Value>,
  step: v8::Local<'a, v8::Function>,
  result: Option<v8::Local<'a, v8::Function>>,
  inverse: Option<v8::Local<'a, v8::Function>>,
}

impl<'a> AggregateFunctionOption<'a> {
  fn from_value(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, validators::Error> {
    use validators::Error;

    if !value.is_object() {
      return Err(Error::InvalidArgType(
        "The \"options\" argument must be an object.",
      ));
    }

    v8_static_strings! {
      DETERMINISTIC_STRING = "deterministic",
      DIRECT_ONLY_STRING = "directOnly",
      USE_BIG_INT_ARGUMENTS_STRING = "useBigIntArguments",
      VARARGS_STRING = "varargs",
      START_STRING = "start",
      STEP_STRING = "step",
      RESULT_STRING = "result",
      INVERSE_STRING = "inverse",
    }

    let start_key = START_STRING.v8_string(scope).unwrap();
    let start_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, start_key.into())
      .unwrap();
    if start_value.is_undefined() {
      return Err(Error::InvalidArgType(
        "The \"options.start\" argument must be a function or a primitive value.",
      ));
    }

    let step_key = STEP_STRING.v8_string(scope).unwrap();
    let step_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, step_key.into())
      .unwrap();
    let step_function = v8::Local::<v8::Function>::try_from(step_value)
      .map_err(|_| {
        Error::InvalidArgType(
          "The \"options.step\" argument must be a function.",
        )
      })?;

    let result_key = RESULT_STRING.v8_string(scope).unwrap();
    let result_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, result_key.into())
      .unwrap();
    let result_function = if result_value.is_undefined() {
      None
    } else {
      let func =
        v8::Local::<v8::Function>::try_from(result_value).map_err(|_| {
          Error::InvalidArgType(
            "The \"options.result\" argument must be a function.",
          )
        })?;
      Some(func)
    };

    let mut deterministic = false;
    let mut use_big_int_arguments = false;
    let mut varargs = false;
    let mut direct_only = false;

    let deterministic_key = DETERMINISTIC_STRING.v8_string(scope).unwrap();
    let deterministic_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, deterministic_key.into())
      .unwrap();
    if !deterministic_value.is_undefined() {
      if !deterministic_value.is_boolean() {
        return Err(Error::InvalidArgType(
          "The \"options.deterministic\" argument must be a boolean.",
        ));
      }
      deterministic = deterministic_value.boolean_value(scope);
    }

    let use_bigint_key = USE_BIG_INT_ARGUMENTS_STRING.v8_string(scope).unwrap();
    let bigint_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, use_bigint_key.into())
      .unwrap();
    if !bigint_value.is_undefined() {
      if !bigint_value.is_boolean() {
        return Err(Error::InvalidArgType(
          "The \"options.useBigIntArguments\" argument must be a boolean.",
        ));
      }
      use_big_int_arguments = bigint_value.boolean_value(scope);
    }

    let varargs_key = VARARGS_STRING.v8_string(scope).unwrap();
    let varargs_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, varargs_key.into())
      .unwrap();
    if !varargs_value.is_undefined() {
      if !varargs_value.is_boolean() {
        return Err(Error::InvalidArgType(
          "The \"options.varargs\" argument must be a boolean.",
        ));
      }
      varargs = varargs_value.boolean_value(scope);
    }

    let direct_only_key = DIRECT_ONLY_STRING.v8_string(scope).unwrap();
    let direct_only_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, direct_only_key.into())
      .unwrap();
    if !direct_only_value.is_undefined() {
      if !direct_only_value.is_boolean() {
        return Err(Error::InvalidArgType(
          "The \"options.directOnly\" argument must be a boolean.",
        ));
      }
      direct_only = direct_only_value.boolean_value(scope);
    }

    let inverse_key = INVERSE_STRING.v8_string(scope).unwrap();
    let inverse_value = v8::Local::<v8::Object>::try_from(value)
      .unwrap()
      .get(scope, inverse_key.into())
      .unwrap();
    let inverse_function = if inverse_value.is_undefined() {
      None
    } else {
      let func =
        v8::Local::<v8::Function>::try_from(inverse_value).map_err(|_| {
          Error::InvalidArgType(
            "The \"options.inverse\" argument must be a function.",
          )
        })?;
      Some(func)
    };

    Ok(AggregateFunctionOption {
      deterministic,
      direct_only,
      use_big_int_arguments,
      varargs,
      start: start_value,
      step: step_function,
      result: result_function,
      inverse: inverse_function,
    })
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
    scope: &mut v8::PinScope<'a, '_>,
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
    if let Some(filter) = obj.get(scope, filter_string.into())
      && !filter.is_undefined()
    {
      if !filter.is_function() {
        return Err(Error::InvalidArgType(
          "The \"options.filter\" argument must be a function.",
        ));
      }

      options.filter = Some(filter);
    }

    let on_conflict_string = ON_CONFLICT_STRING.v8_string(scope).unwrap();
    if let Some(on_conflict) = obj.get(scope, on_conflict_string.into())
      && !on_conflict.is_undefined()
    {
      if !on_conflict.is_function() {
        return Err(Error::InvalidArgType(
          "The \"options.onConflict\" argument must be a function.",
        ));
      }

      options.on_conflict = Some(on_conflict);
    }

    Ok(Some(options))
  }
}

pub struct DatabaseSync {
  pub conn: Rc<RefCell<Option<rusqlite::Connection>>>,
  statements: Rc<RefCell<Vec<InnerStatementPtr>>>,
  options: DatabaseSyncOptions,
  location: String,
  ignore_next_sqlite_error: Rc<Cell<bool>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for DatabaseSync {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

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
  location: &str,
  options: &DatabaseSyncOptions,
) -> Result<rusqlite::Connection, SqliteError> {
  let perms = state.borrow::<PermissionsContainer>();
  let disable_attach = perms
    .check_has_all_permissions(Path::new(location))
    .is_err();

  if location == ":memory:" {
    let conn = rusqlite::Connection::open_in_memory()?;
    if disable_attach {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE,
        false
      ));
      conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    }

    if options.allow_extension {
      perms.check_ffi_all()?;
    } else {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
        false
      ));
    }

    return Ok(conn);
  }

  let location = perms
    .check_open(
      Cow::Borrowed(Path::new(location)),
      match options.read_only {
        true => OpenAccessKind::ReadNoFollow,
        false => OpenAccessKind::ReadWriteNoFollow,
      },
      Some("node:sqlite"),
    )?
    .into_path();

  if options.read_only {
    let conn = rusqlite::Connection::open_with_flags(
      location,
      rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    if disable_attach {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE,
        false
      ));
      conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    }

    if options.allow_extension {
      perms.check_ffi_all()?;
    } else {
      assert!(set_db_config(
        &conn,
        SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
        false
      ));
    }

    return Ok(conn);
  }

  let conn = rusqlite::Connection::open(location)?;
  conn.busy_timeout(std::time::Duration::from_millis(options.timeout))?;

  if options.allow_extension {
    perms.check_ffi_all()?;
  } else {
    assert!(set_db_config(
      &conn,
      SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
      false
    ));
  }

  if disable_attach {
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
  }

  Ok(conn)
}

fn is_open(
  scope: &mut v8::PinScope<'_, '_>,
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
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[string] location: String,
    #[from_v8] options: DatabaseSyncOptions,
  ) -> Result<DatabaseSync, SqliteError> {
    let db = if options.open {
      let db = open_db(state, &location, &options)?;

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
      ignore_next_sqlite_error: Rc::new(Cell::new(false)),
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

    let db = open_db(state, &self.location, &self.options)?;
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
      match stmt.get() {
        None => continue,
        Some(ptr) => {
          // SAFETY: `ptr` is a valid statement handle.
          unsafe {
            libsqlite3_sys::sqlite3_finalize(ptr);
          };
          stmt.set(None);
        }
      };
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

    if let Err(err) = db.execute_batch(sql) {
      if self.consume_ignore_next_sqlite_error() {
        return Ok(());
      }
      return Err(err.into());
    }

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
    check_error_code(r, raw_handle)?;

    let stmt_cell = Rc::new(Cell::new(Some(raw_stmt)));
    self.statements.borrow_mut().push(stmt_cell.clone());

    Ok(StatementSync {
      inner: stmt_cell,
      db: Rc::downgrade(&self.conn),
      statements: Rc::clone(&self.statements),
      ignore_next_sqlite_error: Rc::clone(&self.ignore_next_sqlite_error),
      use_big_ints: Cell::new(false),
      allow_bare_named_params: Cell::new(true),
      allow_unknown_named_params: Cell::new(false),
      is_iter_finished: Cell::new(false),
    })
  }

  #[fast]
  #[validate(is_open)]
  #[undefined]
  fn function<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments>,
  ) -> Result<(), SqliteError> {
    let Some(args) = args.filter(|args| args.length() > 0) else {
      return Err(
        validators::Error::InvalidArgType(
          "The \"name\" argument must be a string.",
        )
        .into(),
      );
    };

    if !args.get(0).is_string() {
      return Err(
        validators::Error::InvalidArgType(
          "The \"name\" argument must be a string.",
        )
        .into(),
      );
    }
    let name = args.get(0).to_rust_string_lossy(scope);

    let (options_value, function_value) = if args.length() < 3 {
      (None, args.get(1))
    } else {
      (Some(args.get(1)), args.get(2))
    };

    let Ok(function) = v8::Local::<v8::Function>::try_from(function_value)
    else {
      return Err(
        validators::Error::InvalidArgType(
          "The \"function\" argument must be a function.",
        )
        .into(),
      );
    };

    let mut use_big_int_arguments = false;
    let mut varargs = false;
    let mut deterministic = false;
    let mut direct_only = false;

    if let Some(value) = options_value
      && !value.is_undefined()
    {
      if value.is_null() || !value.is_object() {
        return Err(
          validators::Error::InvalidArgType(
            "The \"options\" argument must be an object.",
          )
          .into(),
        );
      }

      let options = v8::Local::<v8::Object>::try_from(value).unwrap();

      v8_static_strings! {
        USE_BIG_INT_ARGUMENTS = "useBigIntArguments",
        VARARGS = "varargs",
        DETERMINISTIC = "deterministic",
        DIRECT_ONLY = "directOnly",
      }

      let use_bigint_key = USE_BIG_INT_ARGUMENTS.v8_string(scope).unwrap();
      let bigint_value = options.get(scope, use_bigint_key.into()).unwrap();
      if !bigint_value.is_undefined() {
        if !bigint_value.is_boolean() {
          return Err(
            validators::Error::InvalidArgType(
              "The \"options.useBigIntArguments\" argument must be a boolean.",
            )
            .into(),
          );
        }
        use_big_int_arguments = bigint_value.boolean_value(scope);
      }

      let varargs_key = VARARGS.v8_string(scope).unwrap();
      let varargs_value = options.get(scope, varargs_key.into()).unwrap();
      if !varargs_value.is_undefined() {
        if !varargs_value.is_boolean() {
          return Err(
            validators::Error::InvalidArgType(
              "The \"options.varargs\" argument must be a boolean.",
            )
            .into(),
          );
        }
        varargs = varargs_value.boolean_value(scope);
      }

      let deterministic_key = DETERMINISTIC.v8_string(scope).unwrap();
      let deterministic_value =
        options.get(scope, deterministic_key.into()).unwrap();
      if !deterministic_value.is_undefined() {
        if !deterministic_value.is_boolean() {
          return Err(
            validators::Error::InvalidArgType(
              "The \"options.deterministic\" argument must be a boolean.",
            )
            .into(),
          );
        }
        deterministic = deterministic_value.boolean_value(scope);
      }

      let direct_only_key = DIRECT_ONLY.v8_string(scope).unwrap();
      let direct_only_value =
        options.get(scope, direct_only_key.into()).unwrap();
      if !direct_only_value.is_undefined() {
        if !direct_only_value.is_boolean() {
          return Err(
            validators::Error::InvalidArgType(
              "The \"options.directOnly\" argument must be a boolean.",
            )
            .into(),
          );
        }
        direct_only = direct_only_value.boolean_value(scope);
      }
    }

    v8_static_strings! {
      LENGTH = "length",
    }

    let argc = if varargs {
      -1
    } else {
      let length_key = LENGTH.v8_string(scope).unwrap();
      let length = function.get(scope, length_key.into()).unwrap();
      length.int32_value(scope).unwrap_or(0)
    };

    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let raw_handle = unsafe { db.handle() };
    let name_cstring = CString::new(name)?;

    let callback = v8::Global::new(scope, function).into_raw();
    let context =
      v8::Global::new(scope, scope.get_current_context()).into_raw();

    let data = Box::new(CustomFunctionData {
      callback,
      context,
      use_big_int_arguments,
      ignore_next_sqlite_error: Rc::clone(&self.ignore_next_sqlite_error),
    });
    let data_ptr = Box::into_raw(data);

    let mut text_rep = libsqlite3_sys::SQLITE_UTF8;
    if deterministic {
      text_rep |= libsqlite3_sys::SQLITE_DETERMINISTIC;
    }
    if direct_only {
      text_rep |= libsqlite3_sys::SQLITE_DIRECTONLY;
    }

    // SAFETY: `raw_handle` is a valid database handle.
    // `data_ptr` points to a valid memory location.
    // The v8 handles that are held in `CustomFunctionData` will be
    // dropped when the data is destroyed via `custom_function_destroy`.
    let result = unsafe {
      libsqlite3_sys::sqlite3_create_function_v2(
        raw_handle,
        name_cstring.as_ptr(),
        argc,
        text_rep,
        data_ptr as *mut c_void,
        Some(custom_function_handler),
        None,
        None,
        Some(custom_function_destroy),
      )
    };
    check_error_code(result, raw_handle)?;

    Ok(())
  }

  // Applies a changeset to the database.
  //
  // This method is a wrapper around `sqlite3changeset_apply()`.
  #[fast]
  #[reentrant]
  fn apply_changeset<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[validate(validators::changeset_buffer)]
    #[buffer]
    changeset: &[u8],
    options: v8::Local<'a, v8::Value>,
  ) -> Result<bool, SqliteError> {
    let options = ApplyChangesetOptions::from_value(scope, options)?;

    struct HandlerCtx<'a, 'b, 'c> {
      scope: &'a mut v8::PinScope<'b, 'c>,
      confict: Option<v8::Local<'b, v8::Function>>,
      filter: Option<v8::Local<'b, v8::Function>>,
    }

    // Conflict handler callback for `sqlite3changeset_apply()`.
    unsafe extern "C" fn conflict_handler(
      p_ctx: *mut c_void,
      e_conflict: i32,
      _: *mut libsqlite3_sys::sqlite3_changeset_iter,
    ) -> i32 {
      #[allow(clippy::undocumented_unsafe_blocks)]
      unsafe {
        let ctx = &mut *(p_ctx as *mut HandlerCtx);

        if let Some(conflict) = &mut ctx.confict {
          let recv = v8::undefined(ctx.scope).into();
          let args = [v8::Integer::new(ctx.scope, e_conflict).into()];

          v8::tc_scope!(tc_scope, ctx.scope);

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
    }

    // Filter handler callback for `sqlite3changeset_apply()`.
    unsafe extern "C" fn filter_handler(
      p_ctx: *mut c_void,
      z_tab: *const c_char,
    ) -> i32 {
      #[allow(clippy::undocumented_unsafe_blocks)]
      unsafe {
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

  fn location<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    name_value: v8::Local<'a, v8::Value>,
  ) -> Result<v8::Local<'a, v8::Value>, SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    let name = if !name_value.is_undefined() {
      if !name_value.is_string() {
        return Err(SqliteError::Validation(
          validators::Error::InvalidArgType(
            "The \"dbName\" argument must be a string.",
          ),
        ));
      }
      name_value.to_rust_string_lossy(scope)
    } else {
      "main".to_string()
    };

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let raw_handle = unsafe { db.handle() };
    let name_cstring = CString::new(name)?;

    // SAFETY: `raw_handle` is a valid sqlite3 pointer.
    let db_filename =
      unsafe { sqlite3_db_filename(raw_handle, name_cstring.as_ptr()) };
    if db_filename.is_null() {
      return Ok(v8::null(scope).into());
    }

    // SAFETY: `db_filename` is a valid C string pointer.
    let filename_cstr = unsafe { CStr::from_ptr(db_filename).to_bytes() };
    if filename_cstr.is_empty() {
      return Ok(v8::null(scope).into());
    }

    let filename = v8::String::new_from_utf8(
      scope,
      filename_cstr,
      v8::NewStringType::Normal,
    )
    .unwrap();

    Ok(filename.into())
  }

  #[getter]
  fn is_open(&self) -> bool {
    self.conn.borrow().is_some()
  }

  #[getter]
  fn is_transaction(&self) -> Result<bool, SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::AlreadyClosed)?;

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let res = unsafe { libsqlite3_sys::sqlite3_get_autocommit(db.handle()) };
    Ok(res == 0)
  }

  #[fast]
  #[validate(is_open)]
  fn aggregate<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[validate(validators::name_str)]
    #[string]
    name: &str,
    options: v8::Local<'a, v8::Value>,
  ) -> Result<(), SqliteError> {
    use std::cmp;

    let options = AggregateFunctionOption::from_value(scope, options)?;

    let mut argc = -1;
    if !options.varargs {
      v8_static_strings! {
        LENGTH = "length",
      }
      let length_key = LENGTH.v8_string(scope).unwrap();
      let step_fn_length = options
        .step
        .get(scope, length_key.into())
        .unwrap()
        .int32_value(scope)
        .unwrap();

      // Subtract 1 because the first argument is the aggregate value.
      argc = step_fn_length - 1;

      let inverse_fn_length = if let Some(inverse_fn) = &options.inverse {
        inverse_fn
          .get(scope, length_key.into())
          .unwrap()
          .int32_value(scope)
          .unwrap()
      } else {
        0
      };

      argc = cmp::max(argc, cmp::max(inverse_fn_length - 1, 0));
    }

    let mut text_rep = libsqlite3_sys::SQLITE_UTF8;
    if options.deterministic {
      text_rep |= libsqlite3_sys::SQLITE_DETERMINISTIC;
    }
    if options.direct_only {
      text_rep |= libsqlite3_sys::SQLITE_DIRECTONLY;
    }

    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let context =
      v8::Global::new(scope, scope.get_current_context()).into_raw();

    let start = v8::Global::new(scope, options.start).into_raw();

    let step_fn = v8::Global::new(scope, options.step).into_raw();

    let inverse_fn = options
      .inverse
      .map(|inv| v8::Global::new(scope, inv).into_raw());

    let final_fn = options
      .result
      .map(|res| v8::Global::new(scope, res).into_raw());

    let custom_aggregate = Box::new(CustomAggregate {
      context,
      use_big_int_arguments: options.use_big_int_arguments,
      start,
      step_fn,
      inverse_fn,
      final_fn,
      ignore_next_sqlite_error: Rc::clone(&self.ignore_next_sqlite_error),
    });

    let custom_aggregate_ptr = Box::into_raw(custom_aggregate);

    // SAFETY: lifetime of the connection is guaranteed by reference counting.
    let raw_handle = unsafe { db.handle() };
    let name_cstring = CString::new(name)?;

    let (value_callback, inverse_callback) = if options.inverse.is_some() {
      (
        Some(
          custom_aggregate_xvalue
            as unsafe extern "C" fn(*mut libsqlite3_sys::sqlite3_context),
        ),
        Some(
          custom_aggregate_xinverse
            as unsafe extern "C" fn(
              *mut libsqlite3_sys::sqlite3_context,
              i32,
              *mut *mut libsqlite3_sys::sqlite3_value,
            ),
        ),
      )
    } else {
      (None, None)
    };

    // SAFETY: `raw_handle` is a valid database handle.
    // The v8 handles stored in `CustomAggregate` are released in `custom_aggregate_xdestroy`.
    let r = unsafe {
      sqlite3_create_window_function(
        raw_handle,
        name_cstring.as_ptr(),
        argc,
        text_rep,
        custom_aggregate_ptr as *mut c_void,
        Some(custom_aggregate_xstep),
        Some(custom_aggregate_xfinal),
        value_callback,
        inverse_callback,
        Some(custom_aggregate_xdestroy),
      )
    };
    check_error_code(r, raw_handle)?;

    Ok(())
  }
}

#[repr(C)]
struct AggregateData {
  value: Option<NonNull<v8::Value>>,
  is_window: bool,
  initialized: bool,
}

struct CustomAggregate {
  context: NonNull<v8::Context>,
  use_big_int_arguments: bool,
  start: NonNull<v8::Value>,
  step_fn: NonNull<v8::Function>,
  inverse_fn: Option<NonNull<v8::Function>>,
  final_fn: Option<NonNull<v8::Function>>,
  ignore_next_sqlite_error: Rc<Cell<bool>>,
}

enum AggregateStepKind {
  Step,
  Inverse,
}

unsafe fn get_aggregate_data(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  custom_aggregate: &CustomAggregate,
  scope: &mut v8::PinScope<'_, '_>,
) -> Option<*mut AggregateData> {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    let agg_ptr = libsqlite3_sys::sqlite3_aggregate_context(
      ctx,
      std::mem::size_of::<AggregateData>() as i32,
    ) as *mut AggregateData;

    if agg_ptr.is_null() {
      sqlite_result_error(ctx, "Failed to allocate aggregate context");
      return None;
    }

    let agg = &mut *agg_ptr;
    if !agg.initialized {
      let start_local: v8::Local<v8::Value> =
        std::mem::transmute(custom_aggregate.start.as_ptr());

      let start_value = if start_local.is_function() {
        let start_fn: v8::Local<v8::Function> = start_local.try_into().unwrap();
        let recv = v8::null(scope).into();
        match start_fn.call(scope, recv, &[]) {
          Some(val) => val,
          None => {
            custom_aggregate.ignore_next_sqlite_error.set(true);
            sqlite_result_error(ctx, "");
            return None;
          }
        }
      } else {
        start_local
      };

      let global = v8::Global::new(scope, start_value);
      agg.value = Some(global.into_raw());
      agg.initialized = true;
      agg.is_window = false;
    }

    Some(agg_ptr)
  }
}

unsafe fn custom_aggregate_step_base(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  argc: i32,
  argv: *mut *mut libsqlite3_sys::sqlite3_value,
  kind: AggregateStepKind,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    let data_ptr =
      libsqlite3_sys::sqlite3_user_data(ctx) as *mut CustomAggregate;

    if data_ptr.is_null() {
      sqlite_result_error(
        ctx,
        "Internal error: missing custom aggregate context",
      );
      return;
    }

    let data = &*data_ptr;
    let context_local: v8::Local<v8::Context> =
      std::mem::transmute(data.context.as_ptr());

    v8::callback_scope!(unsafe cb_scope, context_local);
    v8::scope!(scope, cb_scope);
    v8::tc_scope!(tc_scope, scope);

    let Some(agg_ptr) = get_aggregate_data(ctx, data, tc_scope) else {
      if tc_scope.has_caught() {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
      }
      return;
    };
    let agg = &mut *agg_ptr;

    let step_fn = match kind {
      AggregateStepKind::Step => {
        let step_fn: v8::Local<v8::Function> =
          std::mem::transmute(data.step_fn.as_ptr());
        step_fn
      }
      AggregateStepKind::Inverse => {
        if let Some(inverse_ptr) = data.inverse_fn {
          let inverse_fn: v8::Local<v8::Function> =
            std::mem::transmute(inverse_ptr.as_ptr());
          inverse_fn
        } else {
          sqlite_result_error(ctx, "Internal error: inverse function not set");
          return;
        }
      }
    };

    let argc_len = usize::try_from(argc).unwrap_or(0);
    let args_slice = if argc_len == 0 {
      &[]
    } else {
      std::slice::from_raw_parts(argv, argc_len)
    };

    let current_value = if let Some(value_ptr) = agg.value {
      let global = v8::Global::from_raw(tc_scope, value_ptr);
      let local = v8::Local::new(tc_scope, &global);
      std::mem::forget(global);
      local
    } else {
      v8::undefined(tc_scope).into()
    };

    let mut js_args = Vec::with_capacity(args_slice.len() + 1);
    js_args.push(current_value);

    for &value_ptr in args_slice {
      if let Some(arg) =
        sqlite_value_to_v8(tc_scope, value_ptr, data.use_big_int_arguments)
      {
        js_args.push(arg);
      } else {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
        return;
      }
    }

    let recv = v8::undefined(tc_scope).into();
    let result = step_fn
      .call(tc_scope, recv, &js_args)
      .unwrap_or_else(|| v8::undefined(tc_scope).into());

    if tc_scope.has_caught() {
      data.ignore_next_sqlite_error.set(true);
      sqlite_result_error(ctx, "");
      tc_scope.rethrow();
      return;
    }

    if let Some(old_ptr) = agg.value {
      let _ = v8::Global::from_raw(tc_scope, old_ptr);
    }
    let global = v8::Global::new(tc_scope, result);
    agg.value = Some(global.into_raw());
  }
}

unsafe fn custom_aggregate_value_base(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  is_final: bool,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    let data_ptr =
      libsqlite3_sys::sqlite3_user_data(ctx) as *mut CustomAggregate;
    if data_ptr.is_null() {
      sqlite_result_error(
        ctx,
        "Internal error: missing custom aggregate context",
      );
      return;
    }

    let data = &*data_ptr;
    let context_local: v8::Local<v8::Context> =
      std::mem::transmute(data.context.as_ptr());

    v8::callback_scope!(unsafe cb_scope, context_local);
    v8::scope!(scope, cb_scope);
    v8::tc_scope!(tc_scope, scope);

    let Some(agg_ptr) = get_aggregate_data(ctx, data, tc_scope) else {
      if tc_scope.has_caught() {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
      }
      return;
    };
    let agg = &mut *agg_ptr;

    if !is_final {
      agg.is_window = true;
    } else if agg.is_window {
      if let Some(value_ptr) = agg.value.take() {
        let _ = v8::Global::from_raw(tc_scope, value_ptr);
      }
      return;
    }

    let current_value = if let Some(value_ptr) = agg.value {
      let global = v8::Global::from_raw(tc_scope, value_ptr);
      let local = v8::Local::new(tc_scope, &global);
      if is_final {
        agg.value = None;
      } else {
        std::mem::forget(global);
      }
      local
    } else {
      v8::undefined(tc_scope).into()
    };

    let result = if let Some(result_fn_ptr) = data.final_fn {
      let result_fn: v8::Local<v8::Function> =
        std::mem::transmute(result_fn_ptr.as_ptr());
      let ret = result_fn
        .call(tc_scope, v8::null(tc_scope).into(), &[current_value])
        .unwrap_or_else(|| v8::undefined(tc_scope).into());

      if tc_scope.has_caught() {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
        return;
      }
      ret
    } else {
      current_value
    };

    js_value_to_sqlite(tc_scope, ctx, result);
    if tc_scope.has_caught() {
      data.ignore_next_sqlite_error.set(true);
      sqlite_result_error(ctx, "");
      tc_scope.rethrow();
    }
  }
}

unsafe extern "C" fn custom_aggregate_xstep(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  argc: i32,
  argv: *mut *mut libsqlite3_sys::sqlite3_value,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    custom_aggregate_step_base(ctx, argc, argv, AggregateStepKind::Step);
  }
}

unsafe extern "C" fn custom_aggregate_xinverse(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  argc: i32,
  argv: *mut *mut libsqlite3_sys::sqlite3_value,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    custom_aggregate_step_base(ctx, argc, argv, AggregateStepKind::Inverse);
  }
}

unsafe extern "C" fn custom_aggregate_xfinal(
  ctx: *mut libsqlite3_sys::sqlite3_context,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    custom_aggregate_value_base(ctx, true);
  }
}

unsafe extern "C" fn custom_aggregate_xvalue(
  ctx: *mut libsqlite3_sys::sqlite3_context,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    custom_aggregate_value_base(ctx, false);
  }
}

unsafe extern "C" fn custom_aggregate_xdestroy(data: *mut c_void) {
  // SAFETY: `data` is a valid pointer to CustomAggregate.
  // The v8 handles are properly dropped here.
  unsafe {
    let data = Box::from_raw(data as *mut CustomAggregate);
    let context_local: v8::Local<v8::Context> =
      std::mem::transmute(data.context.as_ptr());

    v8::callback_scope!(unsafe cb_scope, context_local);
    v8::scope!(scope, cb_scope);

    let _ = v8::Global::from_raw(scope, data.context);
    let _ = v8::Global::from_raw(scope, data.start);
    let _ = v8::Global::from_raw(scope, data.step_fn);
    if let Some(inverse_ptr) = data.inverse_fn {
      let _ = v8::Global::from_raw(scope, inverse_ptr);
    }
    if let Some(final_ptr) = data.final_fn {
      let _ = v8::Global::from_raw(scope, final_ptr);
    }
  }
}

impl DatabaseSync {
  fn consume_ignore_next_sqlite_error(&self) -> bool {
    self.ignore_next_sqlite_error.replace(false)
  }
}

struct CustomFunctionData {
  callback: NonNull<v8::Function>,
  context: NonNull<v8::Context>,
  use_big_int_arguments: bool,
  ignore_next_sqlite_error: Rc<Cell<bool>>,
}

unsafe extern "C" fn custom_function_handler(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  argc: i32,
  argv: *mut *mut libsqlite3_sys::sqlite3_value,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    let data_ptr =
      libsqlite3_sys::sqlite3_user_data(ctx) as *mut CustomFunctionData;
    if data_ptr.is_null() {
      sqlite_result_error(
        ctx,
        "Internal error: missing custom function context",
      );
      return;
    }

    let data = &*data_ptr;
    let context_local: v8::Local<v8::Context> =
      std::mem::transmute(data.context.as_ptr());

    v8::callback_scope!(unsafe cb_scope, context_local);
    v8::scope!(scope, cb_scope);
    v8::tc_scope!(tc_scope, scope);

    let function_local: v8::Local<v8::Function> =
      std::mem::transmute(data.callback.as_ptr());

    let argc_len = usize::try_from(argc).unwrap_or(0);
    let args_slice = if argc_len == 0 {
      &[]
    } else {
      std::slice::from_raw_parts(argv, argc_len)
    };

    let mut js_args = Vec::with_capacity(args_slice.len());
    for &value_ptr in args_slice {
      if let Some(arg) =
        sqlite_value_to_v8(tc_scope, value_ptr, data.use_big_int_arguments)
      {
        js_args.push(arg);
      } else {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
        return;
      }
    }

    let recv = v8::undefined(tc_scope).into();
    let result = function_local.call(tc_scope, recv, &js_args);
    if tc_scope.has_caught() {
      data.ignore_next_sqlite_error.set(true);
      sqlite_result_error(ctx, "");
      tc_scope.rethrow();
      return;
    }

    if let Some(value) = result {
      js_value_to_sqlite(tc_scope, ctx, value);
      if tc_scope.has_caught() {
        data.ignore_next_sqlite_error.set(true);
        sqlite_result_error(ctx, "");
        tc_scope.rethrow();
      }
    }
  }
}

unsafe extern "C" fn custom_function_destroy(data: *mut c_void) {
  // SAFETY: `data` is a valid pointer to CustomFunctionData.
  // The v8 handles are properly dropped here.
  unsafe {
    let data = Box::from_raw(data as *mut CustomFunctionData);
    let context_local: v8::Local<v8::Context> =
      std::mem::transmute(data.context.as_ptr());

    v8::callback_scope!(unsafe cb_scope, context_local);
    v8::scope!(scope, cb_scope);

    let _ = v8::Global::from_raw(scope, data.callback);
    let _ = v8::Global::from_raw(scope, data.context);
  }
}

fn sqlite_value_to_v8<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: *mut libsqlite3_sys::sqlite3_value,
  use_big_int_arguments: bool,
) -> Option<v8::Local<'a, v8::Value>> {
  // SAFETY: `value` is a valid sqlite3_value pointer.
  unsafe {
    match libsqlite3_sys::sqlite3_value_type(value) {
      libsqlite3_sys::SQLITE_INTEGER => {
        let val = libsqlite3_sys::sqlite3_value_int64(value);
        if use_big_int_arguments {
          Some(v8::BigInt::new_from_i64(scope, val).into())
        } else if (-MAX_SAFE_JS_INTEGER..=MAX_SAFE_JS_INTEGER).contains(&val) {
          Some(v8::Number::new(scope, val as f64).into())
        } else {
          let msg = format!(
            "Value is too large to be represented as a JavaScript number: {}",
            val
          );
          throw_range_error(scope, &msg);
          None
        }
      }
      libsqlite3_sys::SQLITE_FLOAT => {
        let val = libsqlite3_sys::sqlite3_value_double(value);
        Some(v8::Number::new(scope, val).into())
      }
      libsqlite3_sys::SQLITE_TEXT => {
        let len = libsqlite3_sys::sqlite3_value_bytes(value) as usize;
        if len == 0 {
          let text =
            v8::String::new_from_utf8(scope, b"", v8::NewStringType::Normal)
              .unwrap();
          Some(text.into())
        } else {
          let ptr = libsqlite3_sys::sqlite3_value_text(value);
          let slice = std::slice::from_raw_parts(ptr, len);
          let text =
            v8::String::new_from_utf8(scope, slice, v8::NewStringType::Normal)
              .unwrap();
          Some(text.into())
        }
      }
      libsqlite3_sys::SQLITE_BLOB => {
        let len = libsqlite3_sys::sqlite3_value_bytes(value);
        if len == 0 {
          let ab = v8::ArrayBuffer::new(scope, 0);
          let view = v8::Uint8Array::new(scope, ab, 0, 0).unwrap();
          Some(view.into())
        } else {
          let ptr = libsqlite3_sys::sqlite3_value_blob(value) as *const u8;
          let slice = std::slice::from_raw_parts(ptr, len as usize);
          let backing =
            v8::ArrayBuffer::new_backing_store_from_vec(slice.to_vec())
              .make_shared();
          let ab = v8::ArrayBuffer::with_backing_store(scope, &backing);
          let view = v8::Uint8Array::new(scope, ab, 0, len as usize).unwrap();
          Some(view.into())
        }
      }
      libsqlite3_sys::SQLITE_NULL => Some(v8::null(scope).into()),
      _ => Some(v8::undefined(scope).into()),
    }
  }
}

fn js_value_to_sqlite(
  scope: &mut v8::PinScope<'_, '_>,
  ctx: *mut libsqlite3_sys::sqlite3_context,
  value: v8::Local<v8::Value>,
) {
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    if value.is_null_or_undefined() {
      libsqlite3_sys::sqlite3_result_null(ctx);
      return;
    }

    if value.is_number() {
      let number = value.number_value(scope).unwrap_or(0f64);
      libsqlite3_sys::sqlite3_result_double(ctx, number);
      return;
    }

    if value.is_string() {
      let text = value.to_rust_string_lossy(scope);
      libsqlite3_sys::sqlite3_result_text(
        ctx,
        text.as_ptr() as *const _,
        text.len() as i32,
        libsqlite3_sys::SQLITE_TRANSIENT(),
      );
      return;
    }

    if value.is_array_buffer_view() {
      let view: v8::Local<v8::ArrayBufferView> = value.try_into().unwrap();
      let mut data = view.data();
      let mut size = view.byte_length();
      if data.is_null() {
        static EMPTY: [u8; 0] = [];
        data = EMPTY.as_ptr() as *mut _;
        size = 0;
      }
      libsqlite3_sys::sqlite3_result_blob(
        ctx,
        data,
        size as i32,
        libsqlite3_sys::SQLITE_TRANSIENT(),
      );
      return;
    }

    if value.is_big_int() {
      let bigint: v8::Local<v8::BigInt> = value.try_into().unwrap();
      let (int_value, lossless) = bigint.i64_value();
      if !lossless {
        throw_range_error(scope, "BigInt value is too large for SQLite");
        sqlite_result_error(ctx, "");
        return;
      }
      libsqlite3_sys::sqlite3_result_int64(ctx, int_value);
      return;
    }

    if value.is_promise() {
      sqlite_result_error(
        ctx,
        "Asynchronous user-defined functions are not supported",
      );
      return;
    }

    sqlite_result_error(
      ctx,
      "Returned JavaScript value cannot be converted to a SQLite value",
    );
  }
}

fn sqlite_result_error(
  ctx: *mut libsqlite3_sys::sqlite3_context,
  message: &str,
) {
  let msg = CString::new(message).unwrap();
  // SAFETY: `ctx` is a valid sqlite3_context pointer.
  unsafe {
    libsqlite3_sys::sqlite3_result_error(
      ctx,
      msg.as_ptr(),
      msg.as_bytes().len() as i32,
    );
  }
}

fn throw_range_error(scope: &mut v8::PinScope<'_, '_>, message: &str) {
  let msg = v8::String::new(scope, message).unwrap();
  let error = v8::Exception::range_error(scope, msg);

  v8_static_strings!(CODE = "code", ERR_OUT_OF_RANGE = "ERR_OUT_OF_RANGE");
  let code_key = CODE.v8_string(scope).unwrap();
  let code_value = ERR_OUT_OF_RANGE.v8_string(scope).unwrap();
  let error_obj: v8::Local<v8::Object> = error.try_into().unwrap();
  error_obj
    .set(scope, code_key.into(), code_value.into())
    .unwrap();

  scope.throw_exception(error);
}
