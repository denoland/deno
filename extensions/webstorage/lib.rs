// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone)]
struct LocationDataDir(PathBuf);

pub fn init(location_data_dir: Option<PathBuf>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/webstorage",
      "01_webstorage.js",
    ))
    .ops(vec![
      ("op_webstorage_length", op_sync(op_webstorage_length)),
      ("op_webstorage_key", op_sync(op_webstorage_key)),
      ("op_webstorage_set", op_sync(op_webstorage_set)),
      ("op_webstorage_get", op_sync(op_webstorage_get)),
      ("op_webstorage_remove", op_sync(op_webstorage_remove)),
      ("op_webstorage_clear", op_sync(op_webstorage_clear)),
      (
        "op_webstorage_iterate_keys",
        op_sync(op_webstorage_iterate_keys),
      ),
    ])
    .state(move |state| {
      if let Some(location_data_dir) = location_data_dir.clone() {
        state.put(LocationDataDir(location_data_dir));
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
      let path = state.try_borrow::<LocationDataDir>().ok_or_else(|| {
        DomExceptionNotSupportedError::new(
          "LocalStorage is not supported in this context.",
        )
      })?;
      std::fs::create_dir_all(&path.0)?;
      let conn = Connection::open(path.0.join("local_storage"))?;
      conn.execute(
        "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )?;

      state.put(LocalStorage(conn));
    }

    &state.borrow::<LocalStorage>().0
  } else {
    if state.try_borrow::<SessionStorage>().is_none() {
      let conn = Connection::open_in_memory()?;
      conn.execute(
        "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )?;

      state.put(SessionStorage(conn));
    }

    &state.borrow::<SessionStorage>().0
  };

  Ok(conn)
}

pub fn op_webstorage_length(
  state: &mut OpState,
  persistent: bool,
  _: (),
) -> Result<u32, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare("SELECT COUNT(*) FROM data")?;

  let length: u32 = stmt.query_row(params![], |row| row.get(0))?;

  Ok(length)
}

pub fn op_webstorage_key(
  state: &mut OpState,
  index: u32,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare("SELECT key FROM data LIMIT 1 OFFSET ?")?;

  let key: Option<String> = stmt
    .query_row(params![index], |row| row.get(0))
    .optional()?;

  Ok(key)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetArgs {
  key_name: String,
  key_value: String,
}

pub fn op_webstorage_set(
  state: &mut OpState,
  args: SetArgs,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt =
    conn.prepare("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")?;
  let size: u32 = stmt.query_row(params![], |row| row.get(0))?;

  if size >= 5000000 {
    return Err(
      DomExceptionQuotaExceededError::new("Exceeded maximum storage size")
        .into(),
    );
  }

  conn.execute(
    "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
    params![args.key_name, args.key_value],
  )?;

  Ok(())
}

pub fn op_webstorage_get(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<Option<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare("SELECT value FROM data WHERE key = ?")?;

  let val = stmt
    .query_row(params![key_name], |row| row.get(0))
    .optional()?;

  Ok(val)
}

pub fn op_webstorage_remove(
  state: &mut OpState,
  key_name: String,
  persistent: bool,
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  conn.execute("DELETE FROM data WHERE key = ?", params![key_name])?;

  Ok(())
}

pub fn op_webstorage_clear(
  state: &mut OpState,
  persistent: bool,
  _: (),
) -> Result<(), AnyError> {
  let conn = get_webstorage(state, persistent)?;

  conn.execute("DROP TABLE data", params![])?;
  conn.execute(
    "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
    params![],
  )?;

  Ok(())
}

pub fn op_webstorage_iterate_keys(
  state: &mut OpState,
  persistent: bool,
  _: (),
) -> Result<Vec<String>, AnyError> {
  let conn = get_webstorage(state, persistent)?;

  let mut stmt = conn.prepare("SELECT key FROM data")?;

  let keys = stmt
    .query_map(params![], |row| row.get::<_, String>(0))?
    .map(|r| r.unwrap())
    .collect();

  Ok(keys)
}

#[derive(Debug)]
pub struct DomExceptionQuotaExceededError {
  pub msg: String,
}

impl DomExceptionQuotaExceededError {
  pub fn new(msg: &str) -> Self {
    DomExceptionQuotaExceededError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionQuotaExceededError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionQuotaExceededError {}

pub fn get_quota_exceeded_error_class_name(
  e: &AnyError,
) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionQuotaExceededError>()
    .map(|_| "DOMExceptionQuotaExceededError")
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
