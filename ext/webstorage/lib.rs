// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

use std::fmt;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;

pub use rusqlite;

#[derive(Clone)]
struct OriginStorageDir(PathBuf);

const MAX_STORAGE_BYTES: u32 = 10 * 1024 * 1024;

pub fn init(origin_storage_dir: Option<PathBuf>) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec!["deno_webidl"])
    .js(include_js_files!(
      prefix "deno:ext/webstorage",
      "01_webstorage.js",
    ))
    .ops(vec![
      op_webstorage_length::decl(),
      op_webstorage_key::decl(),
      op_webstorage_set::decl(),
      op_webstorage_get::decl(),
      op_webstorage_remove::decl(),
      op_webstorage_clear::decl(),
      op_webstorage_iterate_keys::decl(),
    ])
    .state(move |state| {
      if let Some(origin_storage_dir) = &origin_storage_dir {
        state.put(OriginStorageDir(origin_storage_dir.clone()));
      }
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webstorage.d.ts")
}

struct LocalStorage(Connection);
struct SessionStorage(Connection);

fn get_webstorage(
  state: &mut OpState,
  persistent: bool,
) -> Result<&Connection, AnyError> {
  let conn = if persistent {
    if state.try_borrow::<LocalStorage>().is_none() {
      let path = state.try_borrow::<OriginStorageDir>().ok_or_else(|| {
        DomExceptionNotSupportedError::new(
          "LocalStorage is not supported in this context.",
        )
      })?;
      std::fs::create_dir_all(&path.0)?;
      let conn = Connection::open(path.0.join("local_storage"))?;
      // Enable write-ahead-logging and tweak some other stuff.
      let initial_pragmas = "
        -- enable write-ahead-logging mode
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA temp_store=memory;
        PRAGMA page_size=4096;
        PRAGMA mmap_size=6000000;
        PRAGMA optimize;
      ";

      conn.execute_batch(initial_pragmas)?;
      conn.set_prepared_statement_cache_capacity(128);
      {
        let mut stmt = conn.prepare_cached(
          "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
        )?;
        stmt.execute(params![])?;
      }
      state.put(LocalStorage(conn));
    }

    &state.borrow::<LocalStorage>().0
  } else {
    if state.try_borrow::<SessionStorage>().is_none() {
      let conn = Connection::open_in_memory()?;
      {
        let mut stmt = conn.prepare_cached(
          "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
        )?;
        stmt.execute(params![])?;
      }
      state.put(SessionStorage(conn));
    }

    &state.borrow::<SessionStorage>().0
  };

  Ok(conn)
}

#[op]
pub fn op_webstorage_length(
  state: &mut OpState,
  persistent: bool,
) -> Result<u32, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT COUNT(*) FROM data")?;
  let length: u32 = stmt.query_row(params![], |row| row.get(0))?;

  Ok(length)
}

#[op]
pub fn op_webstorage_key(
  state: &mut OpState,
  index: u32,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt =
    conn.prepare_cached("SELECT key FROM data LIMIT 1 OFFSET ?")?;

  let key: Option<String> = stmt
    .query_row(params![index], |row| row.get(0))
    .optional()?;

  Ok(key)
}

#[op]
pub fn op_webstorage_set(
  state: &mut OpState,
  key: String,
  value: String,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn
    .prepare_cached("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")?;
  let size: u32 = stmt.query_row(params![], |row| row.get(0))?;

  if size >= MAX_STORAGE_BYTES {
    return Err(
      deno_web::DomExceptionQuotaExceededError::new(
        "Exceeded maximum storage size",
      )
      .into(),
    );
  }

  let mut stmt = conn
    .prepare_cached("INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)")?;
  stmt.execute(params![key, value])?;

  Ok(())
}

#[op]
pub fn op_webstorage_get(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT value FROM data WHERE key = ?")?;
  let val = stmt
    .query_row(params![key_name], |row| row.get(0))
    .optional()?;

  Ok(val)
}

#[op]
pub fn op_webstorage_remove(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("DELETE FROM data WHERE key = ?")?;
  stmt.execute(params![key_name])?;

  Ok(())
}

#[op]
pub fn op_webstorage_clear(
  state: &mut OpState,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("DELETE FROM data")?;
  stmt.execute(params![])?;

  Ok(())
}

#[op]
pub fn op_webstorage_iterate_keys(
  state: &mut OpState,
  persistent: bool,
) -> Result<Vec<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT key FROM data")?;
  let keys = stmt
    .query_map(params![], |row| row.get::<_, String>(0))?
    .map(|r| r.unwrap())
    .collect();

  Ok(keys)
}

#[derive(Debug)]
pub struct DomExceptionNotSupportedError {
  pub msg: String,
}

impl DomExceptionNotSupportedError {
  pub fn new(msg: &str) -> Self {
    DomExceptionNotSupportedError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionNotSupportedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionNotSupportedError {}

pub fn get_not_supported_error_class_name(
  e: &AnyError,
) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionNotSupportedError>()
    .map(|_| "DOMExceptionNotSupportedError")
}
