// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::fmt_errors::JSError;
use crate::futures::SinkExt;
use crate::global_state::GlobalState;
use crate::ops::json_op;
use crate::permissions::DenoPermissions;
use crate::startup_data;
use crate::state::State;
use crate::tokio_util::create_basic_runtime;
use crate::web_worker::WebWorker;
use crate::worker::Worker;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core::*;
use futures;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use std;
use std::convert::From;
use std::task::Poll;

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

// TODO(bartlomieju): this function should probably live in `cli/web_worker.rs`
pub fn run_worker_loop(
  rt: &mut tokio::runtime::Runtime,
  worker: &mut Worker,
) -> Result<(), ErrBox> {
  let mut worker_is_ready = false;

  let fut = poll_fn(|cx| -> Poll<Result<(), ErrBox>> {
    if !worker_is_ready {
      match worker.poll_unpin(cx) {
        Poll::Ready(r) => {
          if let Err(e) = r {
            let mut sender = worker.internal_channels.sender.clone();
            futures::executor::block_on(sender.send(WorkerEvent::Error(e)))
              .expect("Failed to post message to host");
          }
          worker_is_ready = true;
        }
        Poll::Pending => {}
      }
    }

    let maybe_msg = {
      match worker.internal_channels.receiver.poll_next_unpin(cx) {
        Poll::Ready(r) => match r {
          Some(msg) => {
            let msg_str = String::from_utf8(msg.to_vec()).unwrap();
            debug!("received message from host: {}", msg_str);
            Some(msg_str)
          }
          None => {
            debug!("channel closed by host, worker event loop shuts down");
            return Poll::Ready(Ok(()));
          }
        },
        Poll::Pending => None,
      }
    };

    if let Some(msg) = maybe_msg {
      // TODO: just add second value and then bind using rusty_v8
      // to get structured clone/transfer working
      let script = format!("workerMessageRecvCallback({})", msg);
      worker
        .execute(&script)
        .expect("Failed to execute message cb");
      // Let worker be polled again
      worker_is_ready = false;
      worker.waker.wake();
    }

    Poll::Pending
  });

  rt.block_on(fut)
}

// TODO(bartlomieju): this function should probably live in `cli/web_worker.rs`
// TODO(bartlomieju): check if order of actions is aligned to Worker spec
fn run_worker_thread(
  name: String,
  global_state: GlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
  has_source_code: bool,
  source_code: String,
) -> Result<WorkerHandle, ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerHandle, ErrBox>>(1);

  let builder =
    std::thread::Builder::new().name(format!("deno-worker-{}", name));
  // TODO(bartlomieju): store JoinHandle as well
  builder.spawn(move || {
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
    run_worker_loop(&mut rt, &mut worker).expect("Panic in event loop");
  })?;

  handle_receiver.recv().unwrap()
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

  let worker_handle = run_worker_thread(
    worker_name,
    global_state,
    permissions,
    module_specifier,
    has_source_code,
    source_code,
  )?;
  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function call
  let worker_id = parent_state.add_child_worker(worker_handle);

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
  let worker_handle =
    state.workers.remove(&id).expect("No worker handle found");
  worker_handle.terminate();
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
  let state_ = state.borrow();
  let worker_handle = state_
    .workers
    .get(&id)
    .expect("No worker handle found")
    .clone();
  let state_ = state.clone();
  let op = async move {
    let response = match worker_handle.get_event().await {
      Some(event) => serialize_worker_event(event),
      None => {
        let mut state_ = state_.borrow_mut();
        let mut handle =
          state_.workers.remove(&id).expect("No worker handle found");
        handle.sender.close_channel();
        // TODO(bartlomieju): join thread handle here
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
  let worker_handle = state.workers.get(&id).expect("No worker handle found");
  let fut = worker_handle
    .post_message(msg)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}
