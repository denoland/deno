// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::*;
use futures;
use futures::Async;
use futures::Future;
use futures::IntoFuture;
use futures::Sink;
use futures::Stream;
use std;
use std::convert::From;
use std::sync::atomic::Ordering;

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
  type Item = Option<Buf>;
  type Error = ErrBox;

  fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
    let mut channels = self.state.worker_channels.lock().unwrap();
    let receiver = &mut channels.receiver;
    receiver.poll().map_err(ErrBox::from)
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

  let op = op
    .map_err(move |_| -> ErrBox { unimplemented!() })
    .and_then(move |maybe_buf| {
      debug!("op_worker_get_message");

      futures::future::ok(json!({
        "data": maybe_buf.map(|buf| buf.to_owned())
      }))
    });

  Ok(JsonOp::Async(Box::new(op)))
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
  sender
    .send(d)
    .wait()
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

  let child_state = ThreadSafeState::new(
    state.global_state.clone(),
    Some(module_specifier.clone()),
    include_deno_namespace,
  )?;
  let rid = child_state.rid;
  let name = format!("USER-WORKER-{}", specifier);
  let deno_main_call = format!("denoMain({})", include_deno_namespace);
  let mut worker =
    Worker::new(name, startup_data::deno_isolate_init(), child_state);
  js_check(worker.execute(&deno_main_call));
  js_check(worker.execute("workerMain()"));

  let exec_cb = move |worker: Worker| {
    let mut workers_tl = parent_state.workers.lock().unwrap();
    workers_tl.insert(rid, worker.shared());
    json!(rid)
  };

  // Has provided source code, execute immediately.
  if has_source_code {
    js_check(worker.execute(&source_code));
    return Ok(JsonOp::Sync(exec_cb(worker)));
  }

  let op = worker
    .execute_mod_async(&module_specifier, None, false)
    .and_then(move |()| Ok(exec_cb(worker)));

  let result = op.wait()?;
  Ok(JsonOp::Sync(result))
}

#[derive(Deserialize)]
struct HostGetWorkerClosedArgs {
  rid: i32,
}

/// Return when the worker closes
fn op_host_get_worker_closed(
  state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetWorkerClosedArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let state = state.clone();

  let shared_worker_future = {
    let workers_tl = state.workers.lock().unwrap();
    let worker = workers_tl.get(&rid).unwrap();
    worker.clone()
  };

  let op =
    shared_worker_future.then(move |_result| futures::future::ok(json!({})));

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct HostGetMessageArgs {
  rid: i32,
}

/// Get message from guest worker as host
fn op_host_get_message(
  _state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetMessageArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let op = Worker::get_message_from_resource(rid)
    .map_err(move |_| -> ErrBox { unimplemented!() })
    .and_then(move |maybe_buf| {
      futures::future::ok(json!({
        "data": maybe_buf.map(|buf| buf.to_owned())
      }))
    });

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct HostPostMessageArgs {
  rid: i32,
}

/// Post message to guest worker as host
fn op_host_post_message(
  _state: &ThreadSafeState,
  args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostPostMessageArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;

  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  // TODO: rename to post_message_to_child(rid, d)
  Worker::post_message_to_resource(rid, d)
    .into_future()
    .wait()
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

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
