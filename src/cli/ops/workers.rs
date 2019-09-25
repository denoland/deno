// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::js_check;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::resources;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::*;
use futures;
use futures::Async;
use futures::Future;
use futures::Sink;
use futures::Stream;
use std;
use std::convert::From;

struct GetMessageFuture {
  pub state: ThreadSafeState,
}

impl Future for GetMessageFuture {
  type Item = Option<Buf>;
  type Error = ();

  fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
    let mut wc = self.state.worker_channels.lock().unwrap();
    wc.1
      .poll()
      .map_err(|err| panic!("worker_channel recv err {:?}", err))
  }
}

/// Get message from host as guest worker
pub fn op_worker_get_message(
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
pub fn op_worker_post_message(
  state: &ThreadSafeState,
  _args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  let tx = {
    let wc = state.worker_channels.lock().unwrap();
    wc.0.clone()
  };
  tx.send(d)
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
pub fn op_create_worker(
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

  let mut module_specifier = ModuleSpecifier::resolve_url_or_path(specifier)?;

  let mut child_argv = parent_state.argv.clone();

  if !has_source_code {
    if let Some(module) = state.main_module() {
      module_specifier =
        ModuleSpecifier::resolve_import(specifier, &module.to_string())?;
      child_argv[1] = module_specifier.to_string();
    }
  }

  let child_state = ThreadSafeState::new(
    parent_state.flags.clone(),
    child_argv,
    parent_state.progress.clone(),
    include_deno_namespace,
  )?;
  let rid = child_state.resource.rid;
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
    .execute_mod_async(&module_specifier, false)
    .and_then(move |()| Ok(exec_cb(worker)));

  let result = op.wait()?;
  Ok(JsonOp::Sync(result))
}

#[derive(Deserialize)]
struct HostGetWorkerClosedArgs {
  rid: i32,
}

/// Return when the worker closes
pub fn op_host_get_worker_closed(
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

  let op = Box::new(
    shared_worker_future.then(move |_result| futures::future::ok(json!({}))),
  );

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct HostGetMessageArgs {
  rid: i32,
}

/// Get message from guest worker as host
pub fn op_host_get_message(
  _state: &ThreadSafeState,
  args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostGetMessageArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let op = resources::get_message_from_worker(rid)
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
pub fn op_host_post_message(
  _state: &ThreadSafeState,
  args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: HostPostMessageArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;

  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  resources::post_message_to_worker(rid, d)
    .wait()
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

  Ok(JsonOp::Sync(json!({})))
}
