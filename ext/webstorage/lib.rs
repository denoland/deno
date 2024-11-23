// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

use deno_core::op2;
use deno_core::OpState;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use std::path::PathBuf;

pub use rusqlite;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebStorageError {
  #[class("DOMExceptionNotSupportedError")]
  #[error("LocalStorage is not supported in this context.")]
  ContextNotSupported,
  #[class(generic)]
  #[error(transparent)]
  Sqlite(#[from] rusqlite::Error),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[inherit] std::io::Error),
  #[class("DOMExceptionQuotaExceededError")]
  #[error("Exceeded maximum storage size")]
  StorageExceeded,
}

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
) -> Result<&Connection, WebStorageError> {
  let conn = if persistent {
    if state.try_borrow::<LocalStorage>().is_none() {
      let path = state
        .try_borrow::<OriginStorageDir>()
        .ok_or(WebStorageError::ContextNotSupported)?;
      std::fs::create_dir_all(&path.0).map_err(WebStorageError::Io)?;
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
) -> Result<u32, WebStorageError> {
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
) -> Result<Option<String>, WebStorageError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt =
    conn.prepare_cached("SELECT key FROM data LIMIT 1 OFFSET ?")?;

  let key: Option<String> = stmt
    .query_row(params![index], |row| row.get(0))
    .optional()?;

  Ok(key)
}

#[inline]
fn size_check(input: usize) -> Result<(), WebStorageError> {
  if input >= MAX_STORAGE_BYTES {
    return Err(WebStorageError::StorageExceeded);
  }

  Ok(())
}

#[op2(fast)]
pub fn op_webstorage_set(
  state: &mut OpState,
  #[string] key: &str,
  #[string] value: &str,
  persistent: bool,
) -> Result<(), WebStorageError> {
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
) -> Result<Option<String>, WebStorageError> {
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
) -> Result<(), WebStorageError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("DELETE FROM data WHERE key = ?")?;
  stmt.execute(params![key_name])?;

  Ok(())
}

#[op2(fast)]
pub fn op_webstorage_clear(
  state: &mut OpState,
  persistent: bool,
) -> Result<(), WebStorageError> {
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
) -> Result<Vec<String>, WebStorageError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare_cached("SELECT key FROM data")?;
  let keys = stmt
    .query_map(params![], |row| row.get::<_, String>(0))?
    .map(|r| r.unwrap())
    .collect();

  Ok(keys)
}
