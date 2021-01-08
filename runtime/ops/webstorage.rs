// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Clone)]
pub struct LocationDataDir(pub Option<PathBuf>);

pub fn init(rt: &mut deno_core::JsRuntime, deno_dir: Option<PathBuf>) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<LocationDataDir>(LocationDataDir(deno_dir));
  }
  super::reg_json_sync(rt, "op_localstorage_open", op_localstorage_open);
  super::reg_json_sync(rt, "op_localstorage_length", op_localstorage_length);
  super::reg_json_sync(rt, "op_localstorage_key", op_localstorage_key);
  super::reg_json_sync(rt, "op_localstorage_set", op_localstorage_set);
  super::reg_json_sync(rt, "op_localstorage_get", op_localstorage_get);
  super::reg_json_sync(rt, "op_localstorage_remove", op_localstorage_remove);
  super::reg_json_sync(rt, "op_localstorage_clear", op_localstorage_clear);
}

struct WebStorageConnectionResource {
  connection: Connection,
}

impl Resource for WebStorageConnectionResource {
  fn name(&self) -> Cow<str> {
    "webStorage".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  session: bool,
}

pub fn op_localstorage_open(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: OpenArgs = serde_json::from_value(args)?;

  if args.session {
    let connection = Connection::open_in_memory().unwrap();
    connection
      .execute(
        "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )
      .unwrap();
    let rid = state
      .resource_table
      .add(WebStorageConnectionResource { connection });
    Ok(json!({ "rid": rid }))
  } else {
    let path = &state.borrow::<LocationDataDir>().0.clone().unwrap();
    std::fs::create_dir_all(&path).unwrap();

    let connection = Connection::open(path.join("local_storage")).unwrap();

    connection
      .execute(
        "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )
      .unwrap();

    let rid = state
      .resource_table
      .add(WebStorageConnectionResource { connection });
    Ok(json!({ "rid": rid }))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LengthArgs {
  rid: u32,
}

pub fn op_localstorage_length(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: LengthArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource
    .connection
    .prepare("SELECT COUNT(*) FROM data")
    .unwrap();

  let length: u32 = stmt.query_row(params![], |row| row.get(0)).unwrap();

  Ok(json!(length))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyArgs {
  rid: u32,
  index: u32,
}

pub fn op_localstorage_key(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: KeyArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource
    .connection
    .prepare("SELECT key FROM data LIMIT 1 OFFSET ?")
    .unwrap();

  let key: Option<String> = stmt
    .query_row(params![args.index], |row| row.get(0))
    .optional()
    .unwrap();

  let json_val = match key {
    Some(string) => json!(string),
    None => Value::Null,
  };

  Ok(json_val)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetArgs {
  rid: u32,
  key_name: String,
  key_value: String,
}

pub fn op_localstorage_set(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: SetArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  resource
    .connection
    .execute(
      "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
      params![args.key_name, args.key_value],
    )
    .unwrap();

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArgs {
  rid: u32,
  key_name: String,
}

pub fn op_localstorage_get(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: GetArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut stmt = resource
    .connection
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let val: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  Ok(json!(val))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveArgs {
  rid: u32,
  key_name: String,
}

pub fn op_localstorage_remove(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RemoveArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  resource
    .connection
    .execute("DELETE FROM data WHERE key = ?", params![args.key_name])
    .unwrap();

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearArgs {
  rid: u32,
}

pub fn op_localstorage_clear(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ClearArgs = serde_json::from_value(args)?;
  let resource = state
    .resource_table
    .get::<WebStorageConnectionResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  resource
    .connection
    .execute("DROP TABLE data", params![])
    .unwrap();
  resource
    .connection
    .execute(
      "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
      params![],
    )
    .unwrap();

  Ok(json!({}))
}
