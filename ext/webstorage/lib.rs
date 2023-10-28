// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

use std::fmt;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;

pub use rusqlite;

#[derive(Clone)]
struct OriginStorageDir(PathBuf);

const MAX_STORAGE_BYTES: usize = 10 * 1024 * 1024;

deno_core::extension!(deno_webstorage,
  deps = [ deno_webidl ],
  ops = [
    op_webstorage_length,
    op_webstorage_key,
    op_webstorage_set,
    op_webstorage_get,
    op_webstorage_remove,
    op_webstorage_clear,
    op_webstorage_iterate_keys,
  ],
  esm = [ "01_webstorage.js" ],
  options = {
    origin_storage_dir: Option<PathBuf>
  },
  state = |state, options| {
    if let Some(origin_storage_dir) = options.origin_storage_dir {
      state.put(OriginStorageDir(origin_storage_dir));
    }
  },
);

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

#[op2(fast)]
pub fn op_webstorage_length(
  state: &mut OpState,
  persistent: bool,
) -> Result<u32, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT COUNT(*) FROM data")?;
  let length: u32 = stmt.query_row(params![], |row| row.get(0))?;

  Ok(length)
}

#[op2]
#[string]
pub fn op_webstorage_key(
  state: &mut OpState,
  #[smi] index: u32,
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

#[inline]
fn size_check(input: usize) -> Result<(), AnyError> {
  if input >= MAX_STORAGE_BYTES {
    return Err(
      deno_web::DomExceptionQuotaExceededError::new(
        "Exceeded maximum storage size",
      )
      .into(),
    );
  }

  Ok(())
}

#[op2(fast)]
pub fn op_webstorage_set(
  state: &mut OpState,
  #[string] key: &str,
  #[string] value: &str,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  size_check(key.len() + value.len())?;

  let mut stmt = conn
    .prepare_cached("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")?;
  let size: u32 = stmt.query_row(params![], |row| row.get(0))?;

  size_check(size as usize)?;

  let mut stmt = conn
    .prepare_cached("INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)")?;
  stmt.execute(params![key, value])?;

  Ok(())
}

#[op2]
#[string]
pub fn op_webstorage_get(
  state: &mut OpState,
  #[string] key_name: String,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT value FROM data WHERE key = ?")?;
  let val = stmt
    .query_row(params![key_name], |row| row.get(0))
    .optional()?;

  Ok(val)
}

#[op2(fast)]
pub fn op_webstorage_remove(
  state: &mut OpState,
  #[string] key_name: &str,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("DELETE FROM data WHERE key = ?")?;
  stmt.execute(params![key_name])?;

  Ok(())
}

#[op2(fast)]
pub fn op_webstorage_clear(
  state: &mut OpState,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("DELETE FROM data")?;
  stmt.execute(params![])?;

  Ok(())
}

#[op2]
#[serde]
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
