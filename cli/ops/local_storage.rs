// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op(
    "op_local_storage_init", s.stateful_json_op(op_local_storage_init));
  i.register_op(
    "op_local_storage_clear", s.stateful_json_op(op_local_storage_clear));
  i.register_op(
    "op_local_storage_get_item",
    s.stateful_json_op(op_local_storage_get_item),
  );
  i.register_op(
    "op_local_storage_get_length",
    s.stateful_json_op(op_local_storage_get_length),
  );
  i.register_op(
    "op_local_storage_set_item",
    s.stateful_json_op(op_local_storage_set_item),
  );
  i.register_op(
    "op_local_storage_remove_item",
    s.stateful_json_op(op_local_storage_remove_item),
  );
}

#[derive(Deserialize)]
struct CreateLocalStorageArgs {
  origin: String,
}

fn op_local_storage_init(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CreateLocalStorageArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  state.local_storage_db = Some(sled::open(args.origin).unwrap());
  drop(state);

  Ok(JsonOp::Sync(json!({})))
}

fn op_local_storage_clear(
  state: &State,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let state = state.borrow();
  state.local_storage_db.as_ref().expect("localStorage must initiated before use")
    .clear().expect("Failed to clear localStorage");
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct AccessLocalStorageArgs {
  key: String,
  value: Option<String>
}

fn op_local_storage_get_item(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: AccessLocalStorageArgs = serde_json::from_value(args)?;
  let key_name = args.key.as_bytes();
  let state = state.borrow();
  let value = state.local_storage_db.as_ref().expect("localStorage must initiated before use").get(key_name).unwrap();
  
  Ok(JsonOp::Sync(
    match value {
      Some(v) => json!({"value": String::from_utf8(v.to_vec()).unwrap()}),
      None => json!({"value":serde_json::Value::Null}),
    }
  ))
}

fn op_local_storage_get_length(
  state: &State,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let state = state.borrow();
  let length = state.local_storage_db.as_ref().expect("localStorage must initiated before use").len();
  Ok(JsonOp::Sync(json!({ "length": length })))
}

/// Get message from guest worker as host
fn op_local_storage_remove_item(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: AccessLocalStorageArgs = serde_json::from_value(args)?;
  let key_name = args.key.as_bytes();
  
  let state = state.borrow();
  state.local_storage_db.as_ref().expect("localStorage must initiated before use")
    .remove(key_name).expect("Failed to remove item from localStorage");

  Ok(JsonOp::Sync(json!({})))
}

/// Post message to guest worker as host
fn op_local_storage_set_item(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: AccessLocalStorageArgs = serde_json::from_value(args)?;
  let key_name = args.key.as_bytes();
  let key_value = args.value.expect("Must provide a value");
  let key_value = key_value.as_bytes();
  
  let state = state.borrow();
  let insertion = state.local_storage_db.as_ref().expect("localStorage must initiated before use").insert(key_name, key_value);

  match insertion {
    Ok(_v) => serde::export::Ok(JsonOp::Sync(json!({}))),
    Err(_e) => serde::export::Ok(JsonOp::Sync(json!({"error": "error"})))
  }

}
