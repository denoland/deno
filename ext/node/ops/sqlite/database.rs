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

#[op2]
impl DatabaseSync {
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

  #[fast]
  fn open(&self) -> Result<(), SqliteError> {
    let db = rusqlite::Connection::open(&self.location)?;
    if self.options.enable_foreign_key_constraints {
      db.execute("PRAGMA foreign_keys = ON", [])?;
    }

    *self.conn.borrow_mut() = Some(db);

    Ok(())
  }

  #[fast]
  fn close(&self) {}

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
      panic!("Failed to prepare statement");
    }

    Ok(StatementSync {
      inner: raw_stmt,
      db: self.conn.clone(),
    })
  }

  #[fast]
  fn exec(&self, #[string] sql: &str) -> Result<(), SqliteError> {
    let db = self.conn.borrow();
    let db = db.as_ref().ok_or(SqliteError::InUse)?;

    let mut stmt = db.prepare_cached(sql)?;
    stmt.raw_execute()?;

    Ok(())
  }
}
