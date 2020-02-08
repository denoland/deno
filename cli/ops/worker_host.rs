// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::fmt_errors::JSError;
use crate::global_state::GlobalState;
use crate::ops::json_op;
use crate::permissions::DenoPermissions;
use crate::startup_data;
use crate::tokio_util::create_basic_runtime;
use crate::state::State;
use crate::web_worker::WebWorker;
use crate::worker::WorkerChannelsExternal;
use deno_core::*;
use futures;
use futures::future::Either;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "create_worker",
    s.core_op(json_op(s.stateful_op(op_create_worker))),
  );
  i.register_op(
    "host_close_worker",
    s.core_op(json_op(s.stateful_op(op_host_close_worker))),
  );
  i.register_op(
    "host_post_message",
    s.core_op(json_op(s.stateful_op(op_host_post_message))),
  );
  i.register_op(
    "host_get_message",
    s.core_op(json_op(s.stateful_op(op_host_get_message))),
  );
  i.register_op("metrics", s.core_op(json_op(s.stateful_op(op_metrics))));
}

fn serialize_worker_result(result: Result<(), ErrBox>) -> Value {
  if let Err(error) = result {
    match error.kind() {
      ErrorKind::JSError => {
        let error = error.downcast::<JSError>().unwrap();
        let exception: V8Exception = error.into();
        json!({"error": {
          "message": exception.message,
          "fileName": exception.script_resource_name,
          "lineNumber": exception.line_number,
          "columnNumber": exception.start_column,
        }})
      }
      _ => json!({"error": {
        "message": error.to_string(),
      }}),
    }
  } else {
    json!({"ok": true})
  }
}

fn create_web_worker(
  name: String,
  global_state: GlobalState,
  permissions: Arc<Mutex<DenoPermissions>>,
  specifier: ModuleSpecifier,
) -> Result<WebWorker, ErrBox> {
  let state = ThreadSafeState::new_for_worker(
    global_state,
    Some(permissions),
    specifier,
  )?;

  let mut worker =
    WebWorker::new(name.to_string(), startup_data::deno_isolate_init(), state);

  // 2. Bootstrap runtime
  let script = format!("bootstrapWorkerRuntime(\"{}\")", name);
  worker.execute(&script)?;

  Ok(worker)
}

// TODO(bartlomieju): check if order of actions is aligned to Worker spec
fn run_worker_thread(
  name: String,
  global_state: GlobalState,
  permissions: Arc<Mutex<DenoPermissions>>,
  specifier: ModuleSpecifier,
  has_source_code: bool,
  source_code: String,
) -> Result<WorkerChannelsExternal, ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerChannelsExternal, ErrBox>>(1);

  // TODO(bartlomieju): should we store JoinHandle as well?
  std::thread::spawn(move || {
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
    // is using `state.internal_worker_channels`.
    //
    // Host can already push messages and interact with worker.
    //
    // Next steps:
    // - create tokio runtime
    // - load provided module or code
    // - start driving worker's event loop

    let mut rt = create_basic_runtime();
    // Execute provided source code immediately
    let result = if has_source_code {
      worker.execute(&source_code)
    } else {
      // TODO(bartlomieju): add "type": "classic", ie. ability to load
      // script instead of module
      let load_future = worker
        .execute_mod_async(&specifier, None, false)
        .boxed_local();

      rt.block_on(load_future)
    };

    if let Err(e) = result {
      let mut_channels = worker.state.worker_channels_internal.lock().unwrap();
      let channels = mut_channels.as_ref().unwrap().clone();
      let msg = serialize_worker_result(Err(e))
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      futures::executor::block_on(channels.post_message(msg))
        .expect("Failed to post message to host");

      // Failing to execute script it terminal error
      return;
    }

    // Drive worker event loop
    let fut = async move {
      loop {
        let receive_msg_fut = {
          let mut_channels =
            worker.state.worker_channels_internal.lock().unwrap();
          let channels = mut_channels.as_ref().unwrap().clone();
          channels.get_message()
        };

        let _result =
          match futures::future::select(&mut *worker, receive_msg_fut).await {
            Either::Left((worker_result, _msg_fut)) => match worker_result {
              Ok(()) => {
                // worker finished scripts, no point in polling it
                // until we receive next message from host (not valid if we add unref
                // ops to worker)
                eprintln!("worker done")
              }
              Err(e) => {
                // serialize and send to host and decide what later -
                // - ie. worker should not be polled unless exception is handled by host
                let result = Err(e);
                let mut_channels =
                  worker.state.worker_channels_internal.lock().unwrap();
                let channels = mut_channels.as_ref().unwrap().clone();
                let msg = serialize_worker_result(result)
                  .to_string()
                  .into_boxed_str()
                  .into_boxed_bytes();
                // TODO: json!({ "type": "error", "error": serialized_error })
                futures::executor::block_on(channels.post_message(msg))
                  .expect("Failed to post message to host");
              }
            },
            Either::Right((maybe_messsage, _worker_fut)) => {
              match maybe_messsage {
                None => {
                  eprintln!("none message received");
                  // TODO: handle if message is none
                }
                Some(msg) => {
                  eprintln!(
                    "message received {}",
                    String::from_utf8(msg.to_vec()).unwrap()
                  );
                  // TODO: just add second value and then bind using rusty_v8
                  // to get structured clone/transfer working
                  let script = format!(
                    "globalThis.workerMessageRecvCallback(\"{}\")",
                    String::from_utf8(msg.to_vec()).unwrap()
                  );
                  worker
                    .execute(&script)
                    .expect("Failed to execute message cb");
                }
              }
            }
          };
      }
    };

    rt.block_on(fut);
  });

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
  let child_permissions = state.permissions.clone();
  let referrer = state.main_module.to_string();
  drop(state);

  let referrer = parent_state.main_module.to_string();
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
    module_specifier.clone(),
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

fn op_host_close_worker(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let mut state = state.borrow_mut();

  let maybe_worker_handle = state.workers.remove(&id);
  if let Some(worker_handle) = maybe_worker_handle {
    let mut sender = worker_handle.sender.clone();
    sender.close_channel();

    let mut receiver =
      futures::executor::block_on(worker_handle.receiver.lock());
    receiver.close();
  };

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct HostGetMessageArgs {
  id: i32,
}

/// Get message from guest worker as host
fn op_host_get_message(
  state: &State,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetMessageArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state = state.borrow();
  // TODO: don't return bad resource anymore
  let worker_handle = state.workers.get(&id).ok_or_else(bad_resource)?;
  let fut = worker_handle.get_message();
  let op = async move {
    let maybe_buf = fut.await;

    // Remove worker if null message
    if maybe_buf.is_none() {
      let mut table = state_.workers.lock().unwrap();
      table.remove(&id);
    }

    Ok(json!({ "data": maybe_buf }))
  };
  Ok(JsonOp::Async(op.boxed_local()))
}

#[derive(Deserialize)]
struct HostPostMessageArgs {
  id: i32,
}

/// Post message to guest worker as host
fn op_host_post_message(
  state: &State,
  args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostPostMessageArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  debug!("post message to worker {}", id);
  let state = state.borrow();
  // TODO: don't return bad resource anymore
  let worker_handle = state.workers.get(&id).ok_or_else(bad_resource)?;
  let fut = worker_handle
    .post_message(msg)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}

fn op_metrics(
  state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let state = state.borrow();
  let m = &state.metrics;

  Ok(JsonOp::Sync(json!({
    "opsDispatched": m.ops_dispatched.load(Ordering::SeqCst) as u64,
    "opsCompleted": m.ops_completed.load(Ordering::SeqCst) as u64,
    "bytesSentControl": m.bytes_sent_control.load(Ordering::SeqCst) as u64,
    "bytesSentData": m.bytes_sent_data.load(Ordering::SeqCst) as u64,
    "bytesReceived": m.bytes_received.load(Ordering::SeqCst) as u64
  })))
}
