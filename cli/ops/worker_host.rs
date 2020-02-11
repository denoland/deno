// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::fmt_errors::JSError;
use crate::ops::json_op;
use crate::state::State;
use crate::web_worker;
use crate::worker::WorkerEvent;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "create_worker",
    s.core_op(json_op(s.stateful_op(op_create_worker))),
  );
  i.register_op(
    "host_terminate_worker",
    s.core_op(json_op(s.stateful_op(op_host_terminate_worker))),
  );
  i.register_op(
    "host_post_message",
    s.core_op(json_op(s.stateful_op(op_host_post_message))),
  );
  i.register_op(
    "host_get_message",
    s.core_op(json_op(s.stateful_op(op_host_get_message))),
  );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkerArgs {
  name: Option<String>,
  specifier: String,
  has_source_code: bool,
  source_code: String,
}

/// Create worker as the host
fn op_create_worker(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CreateWorkerArgs = serde_json::from_value(args)?;

  let specifier = args.specifier.clone();
  let has_source_code = args.has_source_code;
  let source_code = args.source_code.clone();
  let args_name = args.name;
  let parent_state = state.clone();
  let state = state.borrow();
  let global_state = state.global_state.clone();
  let permissions = state.permissions.clone();
  let referrer = state.main_module.to_string();
  drop(state);

  let module_specifier =
    ModuleSpecifier::resolve_import(&specifier, &referrer)?;
  let worker_name = args_name.unwrap_or_else(|| {
    // TODO(bartlomieju): change it to something more descriptive
    format!("USER-WORKER-{}", specifier)
  });

  let (join_handle, worker_handle) = web_worker::run_in_thread(
    worker_name,
    global_state,
    permissions,
    module_specifier,
    has_source_code,
    source_code,
  )?;
  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function call
  let mut parent_state = parent_state.borrow_mut();
  let worker_id = parent_state.next_worker_id;
  parent_state.next_worker_id += 1;
  parent_state
    .workers
    .insert(worker_id, (join_handle, worker_handle));

  Ok(JsonOp::Sync(json!({ "id": worker_id })))
}

#[derive(Deserialize)]
struct WorkerArgs {
  id: i32,
}

fn op_host_terminate_worker(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let mut state = state.borrow_mut();
  let (join_handle, worker_handle) =
    state.workers.remove(&id).expect("No worker handle found");
  worker_handle.terminate();
  join_handle.join().expect("Worker thread panicked");
  Ok(JsonOp::Sync(json!({})))
}

fn serialize_worker_event(event: WorkerEvent) -> Value {
  match event {
    WorkerEvent::Message(buf) => json!({ "type": "msg", "data": buf }),
    WorkerEvent::Error(error) => match error.kind() {
      ErrorKind::JSError => {
        let error = error.downcast::<JSError>().unwrap();
        let exception: V8Exception = error.into();
        json!({
          "type": "error",
          "error": {
            "message": exception.message,
            "fileName": exception.script_resource_name,
            "lineNumber": exception.line_number,
            "columnNumber": exception.start_column,
          }
        })
      }
      _ => json!({
        "type": "error",
        "error": {
          "message": error.to_string(),
        }
      }),
    },
  }
}

/// Get message from guest worker as host
fn op_host_get_message(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let worker_handle = {
    let state_ = state.borrow();
    let (_join_handle, worker_handle) =
      state_.workers.get(&id).expect("No worker handle found");
    worker_handle.clone()
  };

  let state_ = state.clone();
  let op = async move {
    let response = match worker_handle.get_event().await {
      Some(event) => serialize_worker_event(event),
      None => {
        let mut state_ = state_.borrow_mut();
        let (join_handle, mut handle) =
          state_.workers.remove(&id).expect("No worker handle found");
        // Signal shutdown to worker - it should cleanly exit worker event loop.
        handle.sender.close_channel();
        join_handle.join().expect("Worker thread panicked");
        json!({ "type": "close" })
      }
    };
    Ok(response)
  };
  Ok(JsonOp::Async(op.boxed_local()))
}

/// Post message to guest worker as host
fn op_host_post_message(
  state: &State,
  args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  debug!("post message to worker {}", id);
  let state = state.borrow();
  let (_, worker_handle) =
    state.workers.get(&id).expect("No worker handle found");
  let fut = worker_handle
    .post_message(msg)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}
