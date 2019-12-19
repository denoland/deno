// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use rusty_leveldb::{LdbIterator, Options, DB};
use std::path::PathBuf;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "localstorage_get_len",
    s.core_op(json_op(s.stateful_op(op_get_len))),
  );
  i.register_op(
    "localstorage_set_item",
    s.core_op(json_op(s.stateful_op(op_set_item))),
  );
  i.register_op(
    "localstorage_get_item",
    s.core_op(json_op(s.stateful_op(op_get_item))),
  );
  i.register_op(
    "localstorage_remove_item",
    s.core_op(json_op(s.stateful_op(op_remove_item))),
  );

  i.register_op(
    "localstorage_clean",
    s.core_op(json_op(s.stateful_op(op_clean))),
  );

  i.register_op(
    "localstorage_key",
    s.core_op(json_op(s.stateful_op(op_key))),
  );
}

fn op_get_len(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  let mut it = db.new_iter().unwrap();

  let mut length: u8 = 0;

  let (mut key, mut value) = (vec![], vec![]);
  while it.advance() {
    if it.current(&mut key, &mut value) {
      length = length + 1;
    }
  }

  Ok(JsonOp::Sync(json!(length)))
}

#[derive(Deserialize)]
struct SetItemArgs {
  key: std::string::String,
  value: std::string::String,
}

fn op_set_item(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetItemArgs = serde_json::from_value(args)?;

  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  match db.put(args.key.as_bytes(), args.value.as_bytes()) {
    Ok(_) => {}
    Err(e) => return Err(ErrBox::from(e)),
  };

  match db.flush() {
    Ok(_) => {}
    Err(e) => return Err(ErrBox::from(e)),
  }

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct GetItemArgs {
  key: std::string::String,
}

fn op_get_item(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: GetItemArgs = serde_json::from_value(args)?;

  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  let result = db.get(args.key.as_bytes());

  if result.is_none() {
    return Ok(JsonOp::Sync(json!({ "value": Value::Null })));
  }

  let bytes = result.unwrap();

  match String::from_utf8(bytes.clone()) {
    Ok(s) => Ok(JsonOp::Sync(json!({ "value": s }))),
    Err(e) => Err(ErrBox::from(e)), // Invalid UTF-8 sequence
  }
}

#[derive(Deserialize)]
struct DeleteItemArgs {
  key: std::string::String,
}

fn op_remove_item(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DeleteItemArgs = serde_json::from_value(args)?;

  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  match db.delete(args.key.as_bytes()) {
    Ok(_) => {}
    Err(e) => return Err(ErrBox::from(e)),
  }

  match db.flush() {
    Ok(_) => {}
    Err(e) => return Err(ErrBox::from(e)),
  }

  Ok(JsonOp::Sync(json!({})))
}

fn op_clean(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  let mut it = db.new_iter().unwrap();

  let (mut key, mut value) = (vec![], vec![]);
  while it.advance() {
    it.current(&mut key, &mut value);

    match db.delete(&key) {
      Ok(_) => {}
      Err(e) => return Err(ErrBox::from(e)),
    }
  }

  match db.flush() {
    Ok(_) => {}
    Err(e) => return Err(ErrBox::from(e)),
  }

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct KeyItemArgs {
  index: i8,
}

fn op_key(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: KeyItemArgs = serde_json::from_value(args)?;

  let deno_dir = &state.global_state.dir;

  let mut localstorage_file_path: PathBuf = deno_dir.localstorage.clone();
  localstorage_file_path.push("default");

  let mut opt = Options::default();
  opt.create_if_missing = true;
  let mut db = match DB::open(localstorage_file_path.to_str().unwrap(), opt) {
    Ok(db) => db,
    Err(e) => return Err(ErrBox::from(e)),
  };

  let mut it = db.new_iter().unwrap();

  let mut index: i8 = 0;

  let (mut key, mut value) = (vec![], vec![]);
  while it.advance() {
    if it.current(&mut key, &mut value) && index == args.index {
      return Ok(JsonOp::Sync(
        json!({ "value": String::from_utf8(key).unwrap() }),
      ));
    }
    index = index + 1;
  }

  Ok(JsonOp::Sync(json!({ "value": Value::Null })))
}
