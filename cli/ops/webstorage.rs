// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::checksum;
use deno_core::ErrBox;
use deno_core::{OpState, ZeroCopyBuf};
use rusqlite::{params, Connection, OptionalExtension};
use serde_derive::Deserialize;
use serde_json::Value;
use std::rc::Rc;
use std::cell::RefCell;
use deno_core::BufVec;
use tokio::sync::mpsc;
use futures::future::poll_fn;
use std::sync::{Arc, Mutex};


pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_localstorage_open", op_localstorage_open);
  super::reg_json_sync(rt, "op_localstorage_length", op_localstorage_length);
  super::reg_json_sync(rt, "op_localstorage_key", op_localstorage_key);
  super::reg_json_sync(rt, "op_localstorage_set", op_localstorage_set);
  super::reg_json_sync(rt, "op_localstorage_get", op_localstorage_get);
  super::reg_json_sync(rt, "op_localstorage_remove", op_localstorage_remove);
  super::reg_json_sync(rt, "op_localstorage_clear", op_localstorage_clear);
  super::reg_json_async(rt, "op_localstorage_events_poll", op_localstorage_events_poll);
}

struct Event {
  created_by: u32,
  key: Option<String>,
  old_value: Option<String>,
  new_value: Option<String>,
  kind: u32,
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
) -> Result<Value, ErrBox> {
  let args: OpenArgs = serde_json::from_value(args)?;

  if args.session {
    let conn = Connection::open_in_memory().unwrap();
    conn
      .execute(
        "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
        params![],
      )
      .unwrap();
    let session_rid = state.resource_table.add("sessionStorage", Box::new(conn));
    Ok(json!({"rid": session_rid}))
  } else {
    let cli_state = super::cli_state(&state);
    let deno_dir = &cli_state.global_state.dir;
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
        "CREATE TABLE IF NOT EXISTS events (createdBy INT, key VARCHAR, oldValue VARCHAR, newValue VARCHAR, kind INT)",
        params![],
      ) // kind: 0 create, 1 replace, 2 delete, 3 clear
      .unwrap();

    let (sender, receiver) = mpsc::channel::<Value>(16);
    let sender = Mutex::new(sender);
    let mutex_conn = Arc::new(Mutex::new(conn));

    let conn_clone = mutex_conn.clone();

    let foo = mutex_conn.lock().unwrap();
    foo.update_hook(Some(move |_, _, table_name, row_id| {
      if table_name == "events" {
        let mut stmt = conn_clone
          .lock()
          .unwrap()
          .prepare("SELECT * FROM events WHERE rowid = ?")
          .unwrap();

        let event = stmt
          .query_row(params![row_id], |row| {
            Ok(Event {
              created_by: row.get_unwrap(0),
              key: row.get_unwrap(1),
              old_value: row.get_unwrap(2),
              new_value: row.get_unwrap(3),
              kind: row.get_unwrap(4),
            })
          })
          .unwrap();

        if event.created_by != std::process::id() {
          let mut sender = sender.lock().unwrap();

          sender.try_send(json!({
          "key": event.key,
          "oldValue": event.old_value,
          "newValue": event.new_value,
          "kind": event.kind,
        }));
        }
      }
    }));

    let event_rid = state.resource_table.add("localStorageEvents", Box::new(receiver));
    let storage_rid = state.resource_table.add("localStorage", Box::new(mutex_conn));
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
) -> Result<Value, ErrBox> {
  let args: LengthArgs = serde_json::from_value(args)?;
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  let mut stmt = conn.prepare("SELECT COUNT(*) FROM data").unwrap();

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
) -> Result<Value, ErrBox> {
  let args: KeyArgs = serde_json::from_value(args)?;
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  let mut stmt = conn
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
) -> Result<Value, ErrBox> {
  let args: SetArgs = serde_json::from_value(args)?;
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  let mut stmt = conn
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let old_value: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  conn
    .execute(
      "INSERT OR REPLACE INTO data (key, value) VALUES (?, ?)",
      params![args.key_name, args.key_value],
    )
    .unwrap();

  let event_params = match old_value {
    Some(val) => {
      params![std::process::id(), args.key_name, val, args.key_value, 1]
    }
    None => params![
      std::process::id(),
      args.key_name,
      rusqlite::types::Null,
      args.key_value,
      0
    ],
  };

  conn
    .execute(
      "INSERT INTO events (createdBy, key, oldValue, newValue, kind) VALUES (?, ?, ?, ?, ?)",
      event_params,
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
) -> Result<Value, ErrBox> {
  let args: GetArgs = serde_json::from_value(args)?;
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  let mut stmt = conn
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let val: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  let res = match val {
    Some(val) => json!(val),
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
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  let mut stmt = conn
    .prepare("SELECT value FROM data WHERE key = ?")
    .unwrap();

  let old_value: Option<String> = stmt
    .query_row(params![args.key_name], |row| row.get(0))
    .optional()
    .unwrap();

  conn
    .execute("DELETE FROM data WHERE key = ?", params![args.key_name])
    .unwrap();

  if let Some(val) = old_value {
    conn
      .execute(
        "INSERT INTO events (createdBy, key, oldValue, kind) VALUES (?, ?, ?, 2)",
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
) -> Result<Value, ErrBox> {
  let args: ClearArgs = serde_json::from_value(args)?;
  let conn = state
    .resource_table
    .get_mut::<Arc<Mutex<Connection>>>(args.rid)
    .ok_or_else(ErrBox::bad_resource_id)?
    .lock()
    .unwrap();

  conn.execute("DROP data", params![]).unwrap();
  conn
    .execute(
      "CREATE TABLE data (key VARCHAR UNIQUE, value VARCHAR)",
      params![],
    )
    .unwrap();

  conn
    .execute(
      "INSERT INTO events (createdBy, kind) VALUES (?, 3)",
      params![std::process::id()],
    )
    .unwrap();

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventLoopArgs {
  rid: u32,
}

pub async fn op_localstorage_events_poll(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let args: EventLoopArgs = serde_json::from_value(args)?;
  poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let receiver = state
      .resource_table
      .get_mut::<mpsc::Receiver<Value>>(args.rid)
      .ok_or_else(ErrBox::bad_resource_id)?;

    receiver.poll_recv(cx).map(|val| Ok(val.unwrap()))
  })
  .await
}
