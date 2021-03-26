// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone)]
pub struct LocationDataDir(pub Option<PathBuf>);

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  isolate
    .execute(
      "deno:op_crates/webstorage/01_webstorage.js",
      include_str!("01_webstorage.js"),
    )
    .unwrap();
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webstorage.d.ts")
}

struct WebStorageConnectionResource(Connection);

impl Resource for WebStorageConnectionResource {
  fn name(&self) -> Cow<str> {
    "webStorage".into()
  }
}

pub fn op_webstorage_open(
  state: &mut OpState,
  persistent: bool,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<u32, AnyError> {
  let connection = if persistent {
    let path = state.borrow::<LocationDataDir>().0.as_ref().unwrap();
    std::fs::create_dir_all(&path)?;
    Connection::open(path.join("local_storage"))?
  } else {
    Connection::open_in_memory()?
  };

  connection.execute(
    "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
    params![],
  )?;

  let rid = state
    .resource_table
    .add(WebStorageConnectionResource(connection));
  Ok(rid)
}

pub fn op_webstorage_length(
  state: &mut OpState,
  rid: u32,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<u32, AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource.0.prepare("SELECT COUNT(*) FROM data")?;

  let length: u32 = stmt.query_row(params![], |row| row.get(0))?;

  Ok(length)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyArgs {
  rid: u32,
  index: u32,
}

pub fn op_webstorage_key(
  state: &mut OpState,
  args: KeyArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Option<String>, AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource
    .0
    .prepare("SELECT key FROM data LIMIT 1 OFFSET ?")?;

  let key: Option<String> = stmt
    .query_row(params![args.index], |row| row.get(0))
    .optional()?;

  Ok(key)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetArgs {
  rid: u32,
  key_name: String,
  key_value: String,
}

pub fn op_webstorage_set(
  state: &mut OpState,
  args: SetArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource
    .0
    .prepare("SELECT SUM(pgsize) FROM dbstat WHERE name = 'data'")?;
  let size: u32 = stmt.query_row(params![], |row| row.get(0))?;

  if size >= 5000000 {
    return Err(
      DOMExceptionQuotaExceededError::new("Exceeded maximum storage size")
        .into(),
    );
  }

  resource.0.execute(
    "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
    params![args.key_name, args.key_value],
  )?;

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArgs {
  rid: u32,
  key_name: String,
}

pub fn op_webstorage_get(
  state: &mut OpState,
  args: GetArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Option<String>, AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource.0.prepare("SELECT value FROM data WHERE key = ?")?;

  let val = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()?;

  Ok(val)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveArgs {
  rid: u32,
  key_name: String,
}

pub fn op_webstorage_remove(
  state: &mut OpState,
  args: RemoveArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  resource
    .0
    .execute("DELETE FROM data WHERE key = ?", params![args.key_name])?;

  Ok(())
}

pub fn op_webstorage_clear(
  state: &mut OpState,
  rid: u32,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<(), AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(rid)
    .ok_or_else(bad_resource_id)?;

  resource.0.execute("DROP TABLE data", params![])?;
  resource.0.execute(
    "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
    params![],
  )?;

  Ok(())
}

pub fn op_webstorage_iterate_keys(
  state: &mut OpState,
  rid: u32,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Vec<String>, AnyError> {
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource.0.prepare("SELECT key FROM data")?;

  let keys = stmt
    .query_map(params![], |row| row.get::<_, String>(0))?
    .map(|r| r.unwrap())
    .collect();

  Ok(keys)
}

#[derive(Debug)]
pub struct DOMExceptionQuotaExceededError {
  pub msg: String,
}

impl DOMExceptionQuotaExceededError {
  pub fn new(msg: &str) -> Self {
    DOMExceptionQuotaExceededError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DOMExceptionQuotaExceededError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DOMExceptionQuotaExceededError {}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DOMExceptionQuotaExceededError>()
    .map(|_| "DOMExceptionQuotaExceededError")
}
