// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::*;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std;
use std::convert::From;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "create_worker",
    s.core_op(json_op(s.stateful_op(op_create_worker))),
  );
  i.register_op(
    "host_get_worker_closed",
    s.core_op(json_op(s.stateful_op(op_host_get_worker_closed))),
  );
  i.register_op(
    "host_post_message",
    s.core_op(json_op(s.stateful_op(op_host_post_message))),
  );
  i.register_op(
    "host_get_message",
    s.core_op(json_op(s.stateful_op(op_host_get_message))),
  );
  // TODO: make sure these two ops are only accessible to appropriate Worker
  i.register_op(
    "worker_post_message",
    s.core_op(json_op(s.stateful_op(op_worker_post_message))),
  );
  i.register_op(
    "worker_get_message",
    s.core_op(json_op(s.stateful_op(op_worker_get_message))),
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

/// Get message from host as guest worker
fn op_worker_get_message(
  state: &ThreadSafeState,
  _args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let op = GetMessageFuture {
    state: state.clone(),
  };

  let op = op.then(move |maybe_buf| {
    debug!("op_worker_get_message");

    futures::future::ok(json!({
      "data": maybe_buf.map(|buf| buf.to_owned())
    }))
  });

  Ok(JsonOp::Async(op.boxed()))
}

/// Post message to host as guest worker
fn op_worker_post_message(
  state: &ThreadSafeState,
  _args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();
  let mut channels = state.worker_channels.lock().unwrap();
  let sender = &mut channels.sender;
  futures::executor::block_on(sender.send(d))
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkerArgs {
  specifier: String,
  include_deno_namespace: bool,
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
  // Only include deno namespace if requested AND current worker
  // has included namespace (to avoid escalation).
  let include_deno_namespace =
    args.include_deno_namespace && state.include_deno_namespace;
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
    include_deno_namespace,
    int,
  )?;
  // TODO: add a new option to make child worker not sharing permissions
  // with parent (aka .clone(), requests from child won't reflect in parent)
  let name = format!("USER-WORKER-{}", specifier);
  let deno_main_call = format!("denoMain({})", include_deno_namespace);
  let mut worker =
    Worker::new(name, startup_data::deno_isolate_init(), child_state, ext);
  js_check(worker.execute(&deno_main_call));
  js_check(worker.execute("workerMain()"));

  let worker_id = parent_state.add_child_worker(worker.clone());
  let response = json!(worker_id);

  // Has provided source code, execute immediately.
  if has_source_code {
    js_check(worker.execute(&source_code));
    return Ok(JsonOp::Sync(response));
  }

  // TODO(bartlomieju): this should spawn mod execution on separate tokio task
  // and block on receving message on a channel or even use sync channel /shrug
  let (sender, receiver) = mpsc::sync_channel::<Result<(), ErrBox>>(1);
  let fut = worker
    .execute_mod_async(&module_specifier, None, false)
    .then(move |result| {
      sender.send(result).expect("Failed to send message");
      futures::future::ok(())
    })
    .boxed()
    .compat();
  tokio::spawn(fut);

  let result = receiver.recv().expect("Failed to receive message");
  result?;
  Ok(JsonOp::Sync(response))
}

struct GetWorkerClosedFuture {
  state: ThreadSafeState,
  rid: ResourceId,
}

impl Future for GetWorkerClosedFuture {
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

#[derive(Deserialize)]
struct HostGetWorkerClosedArgs {
  id: i32,
}

/// Return when the worker closes
fn op_host_get_worker_closed(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetWorkerClosedArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let state_ = state.clone();

  let future = GetWorkerClosedFuture {
    state: state.clone(),
    rid: id,
  };
  let op = future.then(move |_result| {
    let mut workers_table = state_.workers.lock().unwrap();
    let maybe_worker = workers_table.remove(&id);
    if let Some(worker) = maybe_worker {
      let mut channels = worker.state.worker_channels.lock().unwrap();
      channels.sender.close_channel();
      channels.receiver.close();
    };
    futures::future::ok(json!({}))
  });

  Ok(JsonOp::Async(op.boxed()))
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
  let op = worker
    .get_message()
    .map_err(move |_| -> ErrBox { unimplemented!() })
    .and_then(move |maybe_buf| {
      futures::future::ok(json!({
        "data": maybe_buf.map(|buf| buf.to_owned())
      }))
    });

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
