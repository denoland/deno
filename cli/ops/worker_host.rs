// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use crate::web_worker::run_web_worker;
use crate::web_worker::WebWorker;
use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WorkerEvent;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::channel::mpsc;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::rc::Rc;
use std::thread::JoinHandle;

#[derive(Deserialize)]
struct HostUnhandledErrorArgs {
  message: String,
}

pub fn init(
  rt: &mut deno_core::JsRuntime,
  sender: Option<mpsc::Sender<WorkerEvent>>,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<WorkersTable>(WorkersTable::default());
    state.put::<WorkerId>(WorkerId::default());
  }
  super::reg_json_sync(rt, "op_create_worker", op_create_worker);
  super::reg_json_sync(
    rt,
    "op_host_terminate_worker",
    op_host_terminate_worker,
  );
  super::reg_json_sync(rt, "op_host_post_message", op_host_post_message);
  super::reg_json_async(rt, "op_host_get_message", op_host_get_message);
  super::reg_json_sync(
    rt,
    "op_host_unhandled_error",
    move |_state, args, _zero_copy| {
      if let Some(mut sender) = sender.clone() {
        let args: HostUnhandledErrorArgs = serde_json::from_value(args)?;
        sender
          .try_send(WorkerEvent::Error(generic_error(args.message)))
          .expect("Failed to propagate error event to parent worker");
        Ok(json!(true))
      } else {
        Err(generic_error("Cannot be called from main worker."))
      }
    },
  );
}

pub struct WorkerThread {
  join_handle: JoinHandle<Result<(), AnyError>>,
  worker_handle: WebWorkerHandle,
}

pub type WorkersTable = HashMap<u32, WorkerThread>;
pub type WorkerId = u32;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkerArgs {
  name: Option<String>,
  specifier: String,
  has_source_code: bool,
  source_code: String,
  use_deno_namespace: bool,
}

/// Create worker as the host
fn op_create_worker(
  state: &mut OpState,
  args: Value,
  _data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateWorkerArgs = serde_json::from_value(args)?;

  let specifier = args.specifier.clone();
  let maybe_source_code = if args.has_source_code {
    Some(args.source_code.clone())
  } else {
    None
  };
  let args_name = args.name;
  let use_deno_namespace = args.use_deno_namespace;
  if use_deno_namespace {
    super::check_unstable(state, "Worker.deno");
  }
  let permissions = state.borrow::<Permissions>().clone();
  let worker_id = state.take::<WorkerId>();
  state.put::<WorkerId>(worker_id + 1);

  let module_specifier = ModuleSpecifier::resolve_url(&specifier)?;
  let worker_name = args_name.unwrap_or_else(|| "".to_string());
  let program_state = super::program_state(state);

  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WebWorkerHandle, AnyError>>(1);

  // Setup new thread
  let thread_builder =
    std::thread::Builder::new().name(format!("deno-worker-{}", worker_id));

  // Spawn it
  let join_handle = thread_builder.spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits
    let worker = WebWorker::new(
      worker_name,
      permissions,
      module_specifier.clone(),
      program_state,
      use_deno_namespace,
      worker_id,
    );

    // Send thread safe handle to newly created worker to host thread
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    run_web_worker(worker, module_specifier, maybe_source_code)
  })?;

  let worker_handle = handle_receiver.recv().unwrap()?;

  let worker_thread = WorkerThread {
    join_handle,
    worker_handle,
  };

  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function calls
  state
    .borrow_mut::<WorkersTable>()
    .insert(worker_id, worker_thread);

  Ok(json!({ "id": worker_id }))
}

#[derive(Deserialize)]
struct WorkerArgs {
  id: i32,
}

fn op_host_terminate_worker(
  state: &mut OpState,
  args: Value,
  _data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let worker_thread = state
    .borrow_mut::<WorkersTable>()
    .remove(&id)
    .expect("No worker handle found");
  worker_thread.worker_handle.terminate();
  worker_thread
    .join_handle
    .join()
    .expect("Panic in worker thread")
    .expect("Panic in worker event loop");
  Ok(json!({}))
}

fn serialize_worker_event(event: WorkerEvent) -> Value {
  match event {
    WorkerEvent::Message(buf) => json!({ "type": "msg", "data": buf }),
    WorkerEvent::TerminalError(error) => match error.downcast::<JsError>() {
      Ok(js_error) => json!({
        "type": "terminalError",
        "error": {
          "message": js_error.message,
          "fileName": js_error.script_resource_name,
          "lineNumber": js_error.line_number,
          "columnNumber": js_error.start_column,
        }
      }),
      Err(error) => json!({
        "type": "terminalError",
        "error": {
          "message": error.to_string(),
        }
      }),
    },
    WorkerEvent::Error(error) => match error.downcast::<JsError>() {
      Ok(js_error) => json!({
        "type": "error",
        "error": {
          "message": js_error.message,
          "fileName": js_error.script_resource_name,
          "lineNumber": js_error.line_number,
          "columnNumber": js_error.start_column,
        }
      }),
      Err(error) => json!({
        "type": "error",
        "error": {
          "message": error.to_string(),
        }
      }),
    },
  }
}

/// Try to remove worker from workers table - NOTE: `Worker.terminate()`
/// might have been called already meaning that we won't find worker in
/// table - in that case ignore.
fn try_remove_and_close(state: Rc<RefCell<OpState>>, id: u32) {
  let mut s = state.borrow_mut();
  let workers = s.borrow_mut::<WorkersTable>();
  if let Some(mut worker_thread) = workers.remove(&id) {
    worker_thread.worker_handle.sender.close_channel();
    worker_thread
      .join_handle
      .join()
      .expect("Worker thread panicked")
      .expect("Panic in worker event loop");
  }
}

/// Get message from guest worker as host
async fn op_host_get_message(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;

  let worker_handle = {
    let s = state.borrow();
    let workers_table = s.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      handle.worker_handle.clone()
    } else {
      // If handle was not found it means worker has already shutdown
      return Ok(json!({ "type": "close" }));
    }
  };

  let maybe_event = worker_handle.get_event().await?;
  if let Some(event) = maybe_event {
    // Terminal error means that worker should be removed from worker table.
    if let WorkerEvent::TerminalError(_) = &event {
      try_remove_and_close(state, id);
    }
    return Ok(serialize_worker_event(event));
  }

  // If there was no event from worker it means it has already been closed.
  try_remove_and_close(state, id);
  Ok(json!({ "type": "close" }))
}

/// Post message to guest worker as host
fn op_host_post_message(
  state: &mut OpState,
  args: Value,
  data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  assert_eq!(data.len(), 1, "Invalid number of arguments");
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(&*data[0]).into_boxed_slice();

  debug!("post message to worker {}", id);
  let worker_thread = state
    .borrow::<WorkersTable>()
    .get(&id)
    .expect("No worker handle found");
  worker_thread.worker_handle.post_message(msg)?;
  Ok(json!({}))
}
