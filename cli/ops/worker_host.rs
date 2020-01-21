// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::fmt_errors::JSError;
use crate::ops::json_op;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::web_worker::WebWorker;
use deno_core::*;
use futures;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std;
use std::convert::From;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "create_worker",
    s.core_op(json_op(s.stateful_op(op_create_worker))),
  );
  i.register_op(
    "host_get_worker_loaded",
    s.core_op(json_op(s.stateful_op(op_host_get_worker_loaded))),
  );
  i.register_op(
    "host_poll_worker",
    s.core_op(json_op(s.stateful_op(op_host_poll_worker))),
  );
  i.register_op(
    "host_close_worker",
    s.core_op(json_op(s.stateful_op(op_host_close_worker))),
  );
  i.register_op(
    "host_resume_worker",
    s.core_op(json_op(s.stateful_op(op_host_resume_worker))),
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

struct GetMessageFuture {
  state: ThreadSafeState,
}

impl Future for GetMessageFuture {
  type Output = Option<Buf>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let mut channels = inner.state.worker_channels.lock().unwrap();
    let receiver = &mut channels.receiver;
    receiver.poll_next_unpin(cx)
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkerArgs {
  specifier: String,
  has_source_code: bool,
  source_code: String,
}

/// Create worker as the host
fn op_create_worker(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CreateWorkerArgs = serde_json::from_value(args)?;

  let specifier = args.specifier.as_ref();
  let has_source_code = args.has_source_code;
  let source_code = args.source_code;

  let parent_state = state.clone();

  // TODO(bartlomieju): Isn't this wrong?
  let mut module_specifier = ModuleSpecifier::resolve_url_or_path(specifier)?;
  if !has_source_code {
    if let Some(referrer) = parent_state.main_module.as_ref() {
      let referrer = referrer.clone().to_string();
      module_specifier = ModuleSpecifier::resolve_import(specifier, &referrer)?;
    }
  }

  let (int, ext) = ThreadSafeState::create_channels();
  let child_state = ThreadSafeState::new(
    state.global_state.clone(),
    Some(parent_state.permissions.clone()), // by default share with parent
    Some(module_specifier.clone()),
    int,
  )?;
  // TODO: add a new option to make child worker not sharing permissions
  // with parent (aka .clone(), requests from child won't reflect in parent)
  let name = format!("USER-WORKER-{}", specifier);
  let mut worker =
    WebWorker::new(name, startup_data::deno_isolate_init(), child_state, ext);
  js_check(worker.execute("workerMain()"));

  let worker_id = parent_state.add_child_worker(worker.clone());

  // Has provided source code, execute immediately.
  if has_source_code {
    js_check(worker.execute(&source_code));
    return Ok(JsonOp::Sync(json!({"id": worker_id, "loaded": true})));
  }

  let (mut sender, receiver) = mpsc::channel::<Result<(), ErrBox>>(1);

  // TODO(bartlomieju): this future should be spawned on the separate thread,
  // dedicated to that worker
  let fut = async move {
    let result = worker
      .execute_mod_async(&module_specifier, None, false)
      .await;
    sender.send(result).await.expect("Failed to send message");
  }
  .boxed();
  tokio::spawn(fut);
  let mut table = state.loading_workers.lock().unwrap();
  table.insert(worker_id, receiver);
  Ok(JsonOp::Sync(json!({"id": worker_id, "loaded": false})))
}

struct WorkerPollFuture {
  state: ThreadSafeState,
  rid: ResourceId,
}

impl Future for WorkerPollFuture {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let mut workers_table = inner.state.workers.lock().unwrap();
    let maybe_worker = workers_table.get_mut(&inner.rid);
    if maybe_worker.is_none() {
      return Poll::Ready(Ok(()));
    }
    match maybe_worker.unwrap().poll_unpin(cx) {
      Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
      Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      Poll::Pending => Poll::Pending,
    }
  }
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

#[derive(Deserialize)]
struct WorkerArgs {
  id: i32,
}

fn op_host_get_worker_loaded(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let mut table = state.loading_workers.lock().unwrap();
  let mut receiver = table.remove(&id).unwrap();

  let op = async move {
    let result = receiver.next().await.unwrap();
    Ok(serialize_worker_result(result))
  };

  Ok(JsonOp::Async(op.boxed()))
}

fn op_host_poll_worker(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state_ = state.clone();

  let future = WorkerPollFuture {
    state: state.clone(),
    rid: id,
  };

  let op = async move {
    let result = future.await;

    if result.is_err() {
      let mut workers_table = state_.workers.lock().unwrap();
      let worker = workers_table.get_mut(&id).unwrap();
      worker.clear_exception();
    }

    Ok(serialize_worker_result(result))
  };
  Ok(JsonOp::Async(op.boxed()))
}

fn op_host_close_worker(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state_ = state.clone();

  let mut workers_table = state_.workers.lock().unwrap();
  let maybe_worker = workers_table.remove(&id);
  if let Some(worker) = maybe_worker {
    let mut channels = worker.state.worker_channels.lock().unwrap();
    channels.sender.close_channel();
    channels.receiver.close();
  };

  Ok(JsonOp::Sync(json!({})))
}

fn op_host_resume_worker(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state_ = state.clone();

  let mut workers_table = state_.workers.lock().unwrap();
  let worker = workers_table.get_mut(&id).unwrap();
  js_check(worker.execute("workerMain()"));
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
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetMessageArgs = serde_json::from_value(args)?;

  let id = args.id as u32;
  let mut table = state.workers.lock().unwrap();
  // TODO: don't return bad resource anymore
  let worker = table.get_mut(&id).ok_or_else(bad_resource)?;
  let fut = worker.get_message();

  let op = async move {
    let maybe_buf = fut.await.unwrap();
    Ok(json!({ "data": maybe_buf }))
  };

  Ok(JsonOp::Async(op.boxed()))
}

#[derive(Deserialize)]
struct HostPostMessageArgs {
  id: i32,
}

/// Post message to guest worker as host
fn op_host_post_message(
  state: &ThreadSafeState,
  args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostPostMessageArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  debug!("post message to worker {}", id);
  let mut table = state.workers.lock().unwrap();
  // TODO: don't return bad resource anymore
  let worker = table.get_mut(&id).ok_or_else(bad_resource)?;
  let fut = worker
    .post_message(msg)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()));
  futures::executor::block_on(fut)?;
  Ok(JsonOp::Sync(json!({})))
}

fn op_metrics(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
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
