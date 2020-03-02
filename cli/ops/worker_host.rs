// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::fmt_errors::JSError;
use crate::futures::SinkExt;
use crate::global_state::GlobalState;
use crate::op_error::OpError;
use crate::permissions::DenoPermissions;
use crate::startup_data;
use crate::state::State;
use crate::tokio_util::create_basic_runtime;
use crate::web_worker::WebWorker;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;
use std::thread::JoinHandle;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_create_worker", s.stateful_json_op(op_create_worker));
  i.register_op(
    "op_host_terminate_worker",
    s.stateful_json_op(op_host_terminate_worker),
  );
  i.register_op(
    "op_host_post_message",
    s.stateful_json_op(op_host_post_message),
  );
  i.register_op(
    "op_host_get_message",
    s.stateful_json_op(op_host_get_message),
  );
}

fn create_web_worker(
  name: String,
  global_state: GlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
) -> Result<WebWorker, ErrBox> {
  let state =
    State::new_for_worker(global_state, Some(permissions), specifier)?;

  let mut worker =
    WebWorker::new(name.to_string(), startup_data::deno_isolate_init(), state);
  let script = format!("bootstrapWorkerRuntime(\"{}\")", name);
  worker.execute(&script)?;

  Ok(worker)
}

// TODO(bartlomieju): check if order of actions is aligned to Worker spec
fn run_worker_thread(
  name: String,
  global_state: GlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
  has_source_code: bool,
  source_code: String,
) -> Result<(JoinHandle<()>, WorkerHandle), ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerHandle, ErrBox>>(1);

  let builder =
    std::thread::Builder::new().name(format!("deno-worker-{}", name));
  let join_handle = builder.spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits
    let result =
      create_web_worker(name, global_state, permissions, specifier.clone());

    if let Err(err) = result {
      handle_sender.send(Err(err)).unwrap();
      return;
    }

    let mut worker = result.unwrap();
    // Send thread safe handle to newly created worker to host thread
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    //
    // Next steps:
    // - create tokio runtime
    // - load provided module or code
    // - start driving worker's event loop

    let mut rt = create_basic_runtime();

    // TODO: run with using select with terminate

    // Execute provided source code immediately
    let result = if has_source_code {
      worker.execute(&source_code)
    } else {
      // TODO(bartlomieju): add "type": "classic", ie. ability to load
      // script instead of module
      let load_future = worker.execute_module(&specifier).boxed_local();

      rt.block_on(load_future)
    };

    if let Err(e) = result {
      let mut sender = worker.internal_channels.sender.clone();
      futures::executor::block_on(sender.send(WorkerEvent::Error(e)))
        .expect("Failed to post message to host");

      // Failure to execute script is a terminal error, bye, bye.
      return;
    }

    // TODO(bartlomieju): this thread should return result of event loop
    // that means that we should store JoinHandle to thread to ensure
    // that it actually terminates.
    rt.block_on(worker).expect("Panic in event loop");
  })?;

  let worker_handle = handle_receiver.recv().unwrap()?;
  Ok((join_handle, worker_handle))
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
) -> Result<JsonOp, OpError> {
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

  let (join_handle, worker_handle) = run_worker_thread(
    worker_name,
    global_state,
    permissions,
    module_specifier,
    has_source_code,
    source_code,
  )
  .map_err(|e| OpError::other(e.to_string()))?;
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
) -> Result<JsonOp, OpError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let mut state = state.borrow_mut();
  let (join_handle, worker_handle) =
    state.workers.remove(&id).expect("No worker handle found");
  worker_handle.terminate();
  join_handle.join().expect("Panic in worker thread");
  Ok(JsonOp::Sync(json!({})))
}

fn serialize_worker_event(event: WorkerEvent) -> Value {
  match event {
    WorkerEvent::Message(buf) => json!({ "type": "msg", "data": buf }),
    WorkerEvent::Error(error) => {
      let mut serialized_error = json!({
        "type": "error",
        "error": {
          "message": error.to_string(),
        }
      });

      if let Ok(js_error) = error.downcast::<JSError>() {
        serialized_error = json!({
          "type": "error",
          "error": {
            "message": js_error.message,
            "fileName": js_error.script_resource_name,
            "lineNumber": js_error.line_number,
            "columnNumber": js_error.start_column,
          }
        });
      }

      serialized_error
    }
  }
}

/// Get message from guest worker as host
fn op_host_get_message(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
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
        let (join_handle, mut worker_handle) =
          state_.workers.remove(&id).expect("No worker handle found");
        worker_handle.sender.close_channel();
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
) -> Result<JsonOp, OpError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  debug!("post message to worker {}", id);
  let state = state.borrow();
  let (_, worker_handle) =
    state.workers.get(&id).expect("No worker handle found");
  let fut = worker_handle
    .post_message(msg)
    .map_err(|e| OpError::other(e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}
