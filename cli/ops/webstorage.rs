// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::{ZeroCopyBuf, OpState};
use deno_core::ErrBox;
use serde_derive::Deserialize;
use serde_json::Value;
use crate::checksum;
use rusqlite::{Connection, OptionalExtension, params};

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_localstorage_open", op_localstorage_open);
  super::reg_json_sync(rt, "op_localstorage_length", op_localstorage_length);
  super::reg_json_sync(rt, "op_localstorage_key", op_localstorage_key);
  super::reg_json_sync(rt, "op_localstorage_set", op_localstorage_set);
  super::reg_json_sync(rt, "op_localstorage_get", op_localstorage_get);
  super::reg_json_sync(rt, "op_localstorage_remove", op_localstorage_remove);
  super::reg_json_sync(rt, "op_localstorage_clear", op_localstorage_clear);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  temporary: bool,
  location: String,
}

pub fn op_localstorage_open(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: OpenArgs = serde_json::from_value(args)?;

  let cli_state = super::cli_state(&state);
  let deno_dir = &cli_state.global_state.dir;
  let path = deno_dir.root
    .join("web_storage")
    .join(checksum::gen(&[args.location.as_bytes()]));

  std::fs::create_dir_all(&path).unwrap();

  let conn = Connection::open(path.join("local_storage")).unwrap();

  conn.execute("CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)", params![]).unwrap();

  let rid = state.resource_table.add("localStorage", Box::new(conn));
  Ok(json!(rid))
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
) -> Result<Value, ErrBox> {
  let args: LengthArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let mut stmt = conn.prepare("SELECT COUNT(*) FROM data").unwrap();

  let length: u32 = stmt.query_row(params![], |row| {
    row.get(0)
  }).unwrap();

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
) -> Result<Value, ErrBox> {
  let args: KeyArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let mut stmt = conn.prepare("SELECT key FROM data LIMIT 1 OFFSET ?").unwrap();

  let key: Option<String> = stmt.query_row(&[args.index], |row| {
    row.get(0)
  }).optional().unwrap();


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
) -> Result<Value, ErrBox> {
  let args: SetArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  conn.execute("INSERT or REPLACE INTO data (key, value) values (?1, ?2)", &[args.key_name, args.key_value]).unwrap();

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
) -> Result<Value, ErrBox> {
  let args: GetArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let mut stmt = conn.prepare("SELECT value FROM data WHERE key = ?").unwrap();

  let val: Option<String> = stmt.query_row(params![args.key_name], |row| {
    row.get(0)
  }).optional().unwrap();

  println!("{:?}", val);

  let res = match val {
    Some(value) => json!(value),
    None => Value::Null,
  };

  Ok(res)
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
) -> Result<Value, ErrBox> {
  let args: RemoveArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  conn.execute("DELETE FROM data WHERE key = ?", &[args.key_name]).unwrap();

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
) -> Result<Value, ErrBox> {
  let args: ClearArgs = serde_json::from_value(args)?;
  let conn = state.resource_table
    .get_mut::<Connection>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  conn.execute("DROP data", params![]).unwrap();
  conn.execute("CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)", params![]).unwrap();

  Ok(json!({}))
}

