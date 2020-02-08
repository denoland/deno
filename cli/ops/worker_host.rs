// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::startup_data;
use crate::state::State;
use crate::web_worker::WebWorker;
use crate::worker::WorkerChannelsExternal;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;
use std::sync::atomic::Ordering;

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

  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerChannelsExternal, ErrBox>>(1);

  // TODO(bartlomieju): Isn't this wrong?
  let result = ModuleSpecifier::resolve_url_or_path(&specifier)?;
  let module_specifier = if !has_source_code {
    ModuleSpecifier::resolve_import(&specifier, &referrer)?
  } else {
    result
  };

  std::thread::spawn(move || {
    let result = State::new_for_worker(
      global_state,
      Some(child_permissions), // by default share with parent
      module_specifier.clone(),
    );
    if let Err(err) = result {
      handle_sender.send(Err(err)).unwrap();
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

    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();

    // Has provided source code, execute immediately.
    if has_source_code {
      js_check(worker.execute(&source_code));
      // FIXME(bartlomieju): runtime is not run in this case
      return;
    }

    let fut = async move {
      let r = worker
        .execute_mod_async(&module_specifier, None, false)
        .await;
      if r.is_ok() {
        let _ = (&mut *worker).await;
      }
    }
    .boxed_local();

    crate::tokio_util::run_basic(fut);
  });

  let handle = handle_receiver.recv().unwrap()?;
  let worker_id = parent_state.add_child_worker(handle);

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
