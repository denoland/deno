// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::CString;
use std::ptr::null;
use std::rc::Rc;

use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_permissions::PermissionsContainer;
use serde::Deserialize;

use super::session::SessionOptions;
use super::Session;
use super::SqliteError;
use super::StatementSync;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseSyncOptions {
  #[serde(default = "true_fn")]
  open: bool,
  #[serde(default = "true_fn")]
  enable_foreign_key_constraints: bool,
  read_only: bool,
}

fn true_fn() -> bool {
  true
}

impl Default for DatabaseSyncOptions {
  fn default() -> Self {
    DatabaseSyncOptions {
      open: true,
      enable_foreign_key_constraints: true,
      read_only: false,
    }
  }
}

pub struct DatabaseSync {
  conn: Rc<RefCell<Option<rusqlite::Connection>>>,
  options: DatabaseSyncOptions,
  location: String,
}

impl GarbageCollected for DatabaseSync {}

fn open_db(
  state: &mut OpState,
  readonly: bool,
  location: &str,
) -> Result<rusqlite::Connection, SqliteError> {
  if location == ":memory:" {
    return Ok(rusqlite::Connection::open_in_memory()?);
  }

  state
    .borrow::<PermissionsContainer>()
    .check_read_with_api_name(location, Some("node:sqlite"))?;

  if readonly {
    return Ok(rusqlite::Connection::open_with_flags(
      location,
      rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?);
  }

  state
    .borrow::<PermissionsContainer>()
    .check_write_with_api_name(location, Some("node:sqlite"))?;

  Ok(rusqlite::Connection::open(location)?)
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
    #[serde] options: Option<DatabaseSyncOptions>,
  ) -> Result<DatabaseSync, SqliteError> {
    let options = options.unwrap_or_default();

    let db = if options.open {
      let db = open_db(state, options.read_only, &location)?;

      if options.enable_foreign_key_constraints {
        db.execute("PRAGMA foreign_keys = ON", [])?;
      }
      Some(db)
    } else {
      None
    };

    Ok(DatabaseSync {
      conn: Rc::new(RefCell::new(db)),
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
  fn open(&self, state: &mut OpState) -> Result<(), SqliteError> {
    if self.conn.borrow().is_some() {
      return Err(SqliteError::AlreadyOpen);
    }

    let db = open_db(state, self.options.read_only, &self.location)?;
    if self.options.enable_foreign_key_constraints {
      db.execute("PRAGMA foreign_keys = ON", [])?;
    }

    *self.conn.borrow_mut() = Some(db);

    Ok(())
  }

  // Closes the database connection. An exception is thrown if the
  // database is not open.
  #[fast]
  fn close(&self) -> Result<(), SqliteError> {
    if self.conn.borrow().is_none() {
      return Err(SqliteError::AlreadyClosed);
    }

    *self.conn.borrow_mut() = None;
    Ok(())
  }

  // This method allows one or more SQL statements to be executed
  // without returning any results.
  //
  // This method is a wrapper around sqlite3_exec().
  #[fast]
  fn exec(&self, #[string] sql: &str) -> Result<(), SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let mut stmt = db.prepare_cached(sql)?;
    stmt.raw_execute()?;

    Ok(())
  }

  // Compiles an SQL statement into a prepared statement.
  //
  // This method is a wrapper around `sqlite3_prepare_v2()`.
  #[cppgc]
  fn prepare(&self, #[string] sql: &str) -> Result<StatementSync, SqliteError> {
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

    Ok(StatementSync {
      inner: raw_stmt,
      db: self.conn.clone(),
      use_big_ints: Cell::new(false),
    })
  }

  // Creates and attaches a session to the database.
  //
  // This method is a wrapper around `sqlite3session_create()` and
  // `sqlite3session_attach()`.
  #[cppgc]
  fn create_session(
    &self,
    #[serde] options: Option<SessionOptions>,
  ) -> Result<Session, SqliteError> {
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
      _db: self.conn.clone(),
    })
  }
}
