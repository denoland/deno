// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::checksum;
use deno_core::{BufVec, OpState, ZeroCopyBuf, Resource};
use deno_core::error::AnyError;
use deno_core::error::bad_resource_id;
use deno_core::futures::future::poll_fn;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use std::borrow::Cow;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_localstorage_open", op_localstorage_open);
  super::reg_json_sync(rt, "op_localstorage_length", op_localstorage_length);
  super::reg_json_sync(rt, "op_localstorage_key", op_localstorage_key);
  super::reg_json_sync(rt, "op_localstorage_set", op_localstorage_set);
  super::reg_json_sync(rt, "op_localstorage_get", op_localstorage_get);
  super::reg_json_sync(rt, "op_localstorage_remove", op_localstorage_remove);
  super::reg_json_sync(rt, "op_localstorage_clear", op_localstorage_clear);
  super::reg_json_async(
    rt,
    "op_localstorage_events_poll",
    op_localstorage_events_poll,
  );
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
  location: String,
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
    let session_rid =
      state.resource_table.add(WebStorageConnectionResource { connection });
    Ok(json!({ "rid": session_rid }))
  } else {
    let cli_state = super::global_state(&state);
    let deno_dir = &cli_state.dir;
    let path = deno_dir
      .root
      .join("web_storage")
      .join(checksum::gen(&[args.location.as_bytes()]));

    std::fs::create_dir_all(&path).unwrap();

    let conn = Connection::open(path.join("local_storage")).unwrap();

    conn
      .execute(
        "CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )
      .unwrap();
    conn
      .execute(
        "CREATE TABLE IF NOT EXISTS events (createdBy INT, key VARCHAR, oldValue VARCHAR, newValue VARCHAR)",
        params![],
      )
      .unwrap();

    let (sender, receiver) = mpsc::unbounded_channel::<i64>();

    conn.update_hook(Some(move |_, _: &str, table_name: &str, row_id| {
      if table_name == "events" {
        sender.send(row_id).unwrap();
      }
    }));

    let event_rid = state
      .resource_table
      .add("localStorageEvents", Box::new(receiver));
    let storage_rid = state.resource_table.add("localStorage", Box::new(conn));
    Ok(json!({
      "eventRid": event_rid,
      "rid": storage_rid,
    }))
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

  let mut stmt = resource.connection.prepare("SELECT COUNT(*) FROM data").unwrap();

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

  let mut stmt = resource.connection
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

  let mut stmt = resource.connection
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let old_value: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  resource.connection
    .execute(
      "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
      params![args.key_name, args.key_value],
    )
    .unwrap();

  let pid = std::process::id();
  let insert = |event_params: &[&dyn rusqlite::ToSql]| {
    resource.connection
      .execute(
        "INSERT INTO events (createdBy, key, oldValue, newValue) VALUES (?, ?, ?, ?)",
        event_params,
      )
      .unwrap()
  };

  match old_value {
    Some(val) => insert(params![pid, args.key_name, val, args.key_value]),
    None => insert(params![
      pid,
      args.key_name,
      rusqlite::types::Null,
      args.key_value
    ]),
  };

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

  let mut stmt = resource.connection
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

  let mut stmt = resource.connection
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let old_value: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  resource.connection
    .execute("DELETE FROM data WHERE key = ?", params![args.key_name])
    .unwrap();

  if let Some(val) = old_value {
    resource.connection
      .execute(
        "INSERT INTO events (createdBy, key, oldValue) VALUES (?, ?, ?)",
        params![std::process::id(), args.key_name, val],
      )
      .unwrap();
  }

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

  resource.connection.execute("DROP TABLE data", params![]).unwrap();
  resource.connection
    .execute(
      "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
      params![],
    )
    .unwrap();

  resource.connection
    .execute(
      "INSERT INTO events (createdBy) VALUES (?)",
      params![std::process::id()],
    )
    .unwrap();

  Ok(json!({}))
}

struct Event {
  created_by: u32,
  key: Option<String>,
  old_value: Option<String>,
  new_value: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventLoopArgs {
  event_rid: u32,
  rid: u32,
}

pub async fn op_localstorage_events_poll(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: EventLoopArgs = serde_json::from_value(args)?;
  poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let receiver = state
      .resource_table
      .get_mut::<mpsc::UnboundedReceiver<i64>>(args.event_rid)
      .ok_or_else(bad_resource_id)?;

    receiver.poll_recv(cx).map(|val| {
      let conn = state
        .resource_table
        .get_mut::<Connection>(args.rid)
        .ok_or_else(bad_resource_id)?;

      let mut stmt = conn
        .prepare("SELECT * FROM events WHERE rowid = ?")
        .unwrap();

      let event = stmt
        .query_row(params![val.unwrap()], |row| {
          Ok(Event {
            created_by: row.get_unwrap(0),
            key: row.get_unwrap(1),
            old_value: row.get_unwrap(2),
            new_value: row.get_unwrap(3),
          })
        })
        .unwrap();

      Ok(if event.created_by != std::process::id() {
        json!({
          "key": event.key,
          "oldValue": event.old_value,
          "newValue": event.new_value,
        })
      } else {
        json!({})
      })
    })
  })
  .await
}
