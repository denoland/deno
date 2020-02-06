// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::dispatch_json::JsonResult;
use crate::ops::json_op;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::web_worker::WebWorker;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;
use std::sync::atomic::Ordering;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
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

fn create_web_worker(
  global_state: ThreadSafeGlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
) -> Result<WebWorker, ErrBox> {
  let child_state = ThreadSafeState::new_for_worker(
    global_state,
    Some(permissions),
    module_specifier,
  )?;

  let worker_name = args_name.unwrap_or_else(|| {
    // TODO(bartlomieju): change it to something more descriptive
    format!("USER-WORKER-{}", specifier)
  });

  let mut worker = WebWorker::new(
    worker_name.to_string(),
    startup_data::deno_isolate_init(),
    child_state,
  );

  // 2. Bootstrap runtime
  let script = format!("bootstrapWorkerRuntime(\"{}\")", worker_name);
  worker.execute(&script)?;

  Ok(worker)
}

// TODO(bartlomieju): check if order of actions is aligned to Worker spec
fn run_worker_thread(
  handle_sender: std::sync::mpsc::Sender<JsonResult>,
  global_state: ThreadSafeGlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
) -> Result<u32, ErrBox> {
  // TODO: do it in new thread
  std::thread::spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits
    let result = create_web_worker(global_state, permissions, specifier);

    if let Err(err) = result {
      handle_sender.send(Err(err)).unwrap();
      return;
    }

    let worker = result.unwrap();
    // Send thread safe handle to newly created worker to host thread
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();

    // At this point host can already push messages and interact with worker.
    // Next steps:
    // - execute provided code (optionally)
    // - create tokio runtime
    // - load provided module (optionally)
    // - start driving worker's event loop

    // Execute provided source code immediately
    if has_source_code {
      if let Err(e) = worker.execute(&source_code) {
        handle_sender.send(Err(e)).unwrap();
        return;
      }
    }

    let mut rt = create_basic_runtime();

    // TODO(bartlomieju): add "type": "classic", ie. ability to load
    // script instead of module
    // Load provided module
    if !has_source_code {
      let load_future = worker
        .execute_mod_async(&module_specifier, None, false)
        .boxed_local();

      if let Err(e) = rt.block_on(load_future) {
        handle_sender.send(Err(e)).unwrap();
        return;
      }
    }

    // Drive worker event loop
    loop {
      let a = select!(worker.await, message_receiver.recv().await);

      let result = match a {
        worker_result => match worker_result {
          Ok(()) => {
            // worker finished scripts, no point in polling it
            // until we receive as message (not valid if we add unref 
            // ops to worker)
          },
          Err(e) =>{
            if let Ok(err) = e.downcast::<WorkerCloseError>() {
              // worker shuts down - empty event loop and notify
              // host that this worker is closed
            } else {
              // serialize and send to host and decide what later -
              // - ie. worker should not be polled unless exception is handled by host
            }
          },
        },
        message => {
          // TODO: just add second value and then bind using rusty_v8 
          // to get structured clone/transfer working
          let json_string = "";
          let script = format!("globalThis.workerMessageRecvCallback({})", json_string);
          worker.execute(script).expect("Failed to execute message cb");
          // result
        },
      };

      handle_sender.send(result).unwrap();
    }
  })
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
  state: &ThreadSafeState,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CreateWorkerArgs = serde_json::from_value(args)?;

  let specifier = args.specifier.clone();
  let has_source_code = args.has_source_code;
  let source_code = args.source_code.clone();
  let args_name = args.name;
  let parent_state = state.clone();

  let (load_sender, load_receiver) =
    std::sync::mpsc::sync_channel::<JsonResult>(1);

  let referrer = parent_state.main_module.to_string();
  let module_specifier =
    ModuleSpecifier::resolve_import(&specifier, &referrer)?;

  let worker_id = run_worker_thread()?;
  std::thread::spawn(move || {
    let result = ThreadSafeState::new_for_worker(
      parent_state.global_state.clone(),
      Some(parent_state.permissions.clone()), // by default share with parent
      module_specifier.clone(),
    );
    if let Err(err) = result {
      load_sender.send(Err(err)).unwrap();
      return;
    }
    let child_state = result.unwrap();
    let worker_name = args_name.unwrap_or_else(|| {
      // TODO(bartlomieju): change it to something more descriptive
      format!("USER-WORKER-{}", specifier)
    });

    // TODO: add a new option to make child worker not sharing permissions
    // with parent (aka .clone(), requests from child won't reflect in parent)
    let mut worker = WebWorker::new(
      worker_name.to_string(),
      startup_data::deno_isolate_init(),
      child_state,
    );
    let script = format!("bootstrapWorkerRuntime(\"{}\")", worker_name);
    js_check(worker.execute(&script));
    js_check(worker.execute("runWorkerMessageLoop()"));

    let worker_id = parent_state.add_child_worker(&worker);

    // Has provided source code, execute immediately.
    if has_source_code {
      js_check(worker.execute(&source_code));
      load_sender.send(Ok(json!({ "id": worker_id }))).unwrap();
      return;
    }

    // TODO(bartlomieju): handle errors here and restructure this bit
    let fut = async move {
      let r = worker
        .execute_mod_async(&module_specifier, None, false)
        .await;
      if r.is_ok() {
        let _ = (&mut *worker).await;
      }
    }
    .boxed_local();

    load_sender.send(Ok(json!({ "id": worker_id }))).unwrap();

    crate::tokio_util::run_basic(fut);
  });

  let r = load_receiver.recv().unwrap();

  Ok(JsonOp::Sync(json!({ "id": worker_id })))
}

#[derive(Deserialize)]
struct WorkerArgs {
  id: i32,
}

fn op_host_close_worker(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state_ = state.clone();

  let mut workers_table = state_.workers.lock().unwrap();
  let maybe_worker_handle = workers_table.remove(&id);
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
  state: &ThreadSafeState,
  args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetMessageArgs = serde_json::from_value(args)?;
  let state_ = state.clone();
  let id = args.id as u32;
  let mut table = state_.workers.lock().unwrap();
  // TODO: don't return bad resource anymore
  let worker_handle = table.get_mut(&id).ok_or_else(bad_resource)?;
  let fut = worker_handle.get_message();
  let op = async move {
    let maybe_buf = fut.await;
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
  state: &ThreadSafeState,
  args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostPostMessageArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  debug!("post message to worker {}", id);
  let mut table = state.workers.lock().unwrap();
  // TODO: don't return bad resource anymore
  let worker_handle = table.get_mut(&id).ok_or_else(bad_resource)?;
  let fut = worker_handle
    .post_message(msg)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}

fn op_metrics(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let m = &state.metrics;

  Ok(JsonOp::Sync(json!({
    "opsDispatched": m.ops_dispatched.load(Ordering::SeqCst) as u64,
    "opsCompleted": m.ops_completed.load(Ordering::SeqCst) as u64,
    "bytesSentControl": m.bytes_sent_control.load(Ordering::SeqCst) as u64,
    "bytesSentData": m.bytes_sent_data.load(Ordering::SeqCst) as u64,
    "bytesReceived": m.bytes_received.load(Ordering::SeqCst) as u64
  })))
}
