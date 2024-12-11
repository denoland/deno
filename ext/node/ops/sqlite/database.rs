// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::op2;
use deno_core::GarbageCollected;
use serde::Deserialize;

use super::SqliteError;
use super::StatementSync;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseSyncOptions {
  #[serde(default = "true_fn")]
  open: bool,
  #[serde(default = "true_fn")]
  enable_foreign_key_constraints: bool,
}

fn true_fn() -> bool {
  true
}

impl Default for DatabaseSyncOptions {
  fn default() -> Self {
    DatabaseSyncOptions {
      open: true,
      enable_foreign_key_constraints: true,
    }
  }
}

pub struct DatabaseSync {
  conn: Rc<RefCell<Option<rusqlite::Connection>>>,
  options: DatabaseSyncOptions,
  location: String,
}

impl GarbageCollected for DatabaseSync {}

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
    #[string] location: String,
    #[serde] options: Option<DatabaseSyncOptions>,
  ) -> Result<DatabaseSync, SqliteError> {
    let options = options.unwrap_or_default();

    let db = if options.open {
      let db = rusqlite::Connection::open(&location)?;
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
  fn open(&self) -> Result<(), SqliteError> {
    if self.conn.borrow().is_some() {
      return Err(SqliteError::AlreadyOpen);
    }

    let db = rusqlite::Connection::open(&self.location)?;
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

  #[cppgc]
  fn prepare(&self, #[string] sql: &str) -> Result<StatementSync, SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let raw_handle = unsafe { db.handle() };

    let mut raw_stmt = std::ptr::null_mut();
    let r = unsafe {
      libsqlite3_sys::sqlite3_prepare_v2(
        raw_handle,
        sql.as_ptr() as *const i8,
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
    })
  }
}
