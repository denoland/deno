// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::{ZeroCopyBuf, OpState};
use deno_core::ErrBox;
use serde_derive::Deserialize;
use serde_json::Value;

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
}

pub fn op_localstorage_open(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: OpenArgs = serde_json::from_value(args)?;
  let db = sled::Config::default()
    .path(if args.temporary {"/tmp/sessionStorage"} else {"/tmp/localStorage"})
    .temporary(args.temporary)
    .open().unwrap();
  let rid = state.resource_table.add("localStorage", Box::new(db));
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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let length = db.len();

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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let mut val = None;
  for (index, db_val) in db.iter().enumerate() {
    if (index as u32) == args.index {
      let (key, _) = db_val.unwrap();
      val = Some(String::from_utf8(key.to_vec()).unwrap());
    }
  }

  let json_val = match val {
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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;


  db.insert(&*args.key_name, &*args.key_value).unwrap();

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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  let res = match db.get(&args.key_name).unwrap() {
    Some(value) => json!(String::from_utf8(value.to_vec()).unwrap()),
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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  db.remove(&args.key_name).unwrap();

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
  let db = state.resource_table
    .get_mut::<sled::Db>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?;

  db.clear().unwrap();

  Ok(json!({}))
}

