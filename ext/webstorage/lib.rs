// Copyright 2018-2025 the Deno authors. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

use std::path::PathBuf;

use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::OpState;
pub use rusqlite;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;

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
  Io(std::io::Error),
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
    op_webstorage_iterate_keys,
  ],
  objects = [
    Storage
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

#[inline]
fn size_check(input: usize) -> Result<(), WebStorageError> {
  if input >= MAX_STORAGE_BYTES {
    return Err(WebStorageError::StorageExceeded);
  }

  Ok(())
}

struct Storage {
  persistent: bool,
}

impl GarbageCollected for Storage {}

#[op2]
impl Storage {
  #[constructor]
  #[cppgc]
  fn new(persistent: bool) -> Storage {
    Storage { persistent }
  }

  #[getter]
  #[smi]
  fn length(&self, state: &mut OpState) -> Result<u32, WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    let mut stmt = conn.prepare_cached("SELECT COUNT(*) FROM data")?;
    let length: u32 = stmt.query_row(params![], |row| row.get(0))?;

    Ok(length)
  }

  #[required(1)]
  #[string]
  fn key(
    &self,
    state: &mut OpState,
    #[smi] index: u32,
  ) -> Result<Option<String>, WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    let mut stmt =
      conn.prepare_cached("SELECT key FROM data LIMIT 1 OFFSET ?")?;

    let key: Option<String> = stmt
      .query_row(params![index], |row| row.get(0))
      .optional()?;

    Ok(key)
  }

  #[fast]
  #[required(2)]
  fn set_item(
    &self,
    state: &mut OpState,
    #[string] key: &str,
    #[string] value: &str,
  ) -> Result<(), WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    size_check(key.len() + value.len())?;

    let mut stmt = conn
      .prepare_cached("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")?;
    let size: u32 = stmt.query_row(params![], |row| row.get(0))?;

    size_check(size as usize)?;

    let mut stmt = conn.prepare_cached(
      "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
    )?;
    stmt.execute(params![key, value])?;

    Ok(())
  }

  #[required(1)]
  #[string]
  fn get_item(
    &self,
    state: &mut OpState,
    #[string] key: &str,
  ) -> Result<Option<String>, WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    let mut stmt =
      conn.prepare_cached("SELECT value FROM data WHERE key = ?")?;
    let val = stmt.query_row(params![key], |row| row.get(0)).optional()?;

    Ok(val)
  }

  #[fast]
  #[required(1)]
  fn remove_item(
    &self,
    state: &mut OpState,
    #[string] key: &str,
  ) -> Result<(), WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    let mut stmt = conn.prepare_cached("DELETE FROM data WHERE key = ?")?;
    stmt.execute(params![key])?;

    Ok(())
  }

  #[fast]
  fn clear(&self, state: &mut OpState) -> Result<(), WebStorageError> {
    let conn = get_webstorage(state, self.persistent)?;

    let mut stmt = conn.prepare_cached("DELETE FROM data")?;
    stmt.execute(params![])?;

    Ok(())
  }
}

#[op2]
#[serde]
fn op_webstorage_iterate_keys(
  #[cppgc] storage: &Storage,
  state: &mut OpState,
) -> Result<Vec<String>, WebStorageError> {
  let conn = get_webstorage(state, storage.persistent)?;

  let mut stmt = conn.prepare_cached("SELECT key FROM data")?;
  let keys = stmt
    .query_map(params![], |row| row.get::<_, String>(0))?
    .map(|r| r.unwrap())
    .collect();

  Ok(keys)
}
