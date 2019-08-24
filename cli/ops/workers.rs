// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::utils::ok_buf;
use super::utils::CliOpResult;
use crate::deno_error;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::msg;
use crate::resources;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::*;
use flatbuffers::FlatBufferBuilder;
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
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if base.sync() {
    return Err(deno_error::no_sync_support());
  }
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let op = GetMessageFuture {
    state: state.clone(),
  };
  let op = op.map_err(move |_| -> ErrBox { unimplemented!() });
  let op = op.and_then(move |maybe_buf| -> Result<Buf, ErrBox> {
    debug!("op_worker_get_message");
    let builder = &mut FlatBufferBuilder::new();

    let data = maybe_buf.as_ref().map(|buf| builder.create_vector(buf));
    let inner = msg::WorkerGetMessageRes::create(
      builder,
      &msg::WorkerGetMessageResArgs { data },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::WorkerGetMessageRes,
        ..Default::default()
      },
    ))
  });
  Ok(Op::Async(Box::new(op)))
}

/// Post message to host as guest worker
pub fn op_worker_post_message(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  let cmd_id = base.cmd_id();
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  let tx = {
    let wc = state.worker_channels.lock().unwrap();
    wc.0.clone()
  };
  tx.send(d)
    .wait()
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;
  let builder = &mut FlatBufferBuilder::new();

  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      ..Default::default()
    },
  ))
}

/// Create worker as the host
pub fn op_create_worker(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_create_worker().unwrap();
  let specifier = inner.specifier().unwrap();
  // Only include deno namespace if requested AND current worker
  // has included namespace (to avoid escalation).
  let include_deno_namespace =
    inner.include_deno_namespace() && state.include_deno_namespace;
  let has_source_code = inner.has_source_code();
  let source_code = inner.source_code().unwrap();

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
  worker.execute(&deno_main_call).unwrap();
  worker.execute("workerMain()").unwrap();

  let exec_cb = move |worker: Worker| {
    let mut workers_tl = parent_state.workers.lock().unwrap();
    workers_tl.insert(rid, worker.shared());
    let builder = &mut FlatBufferBuilder::new();
    let msg_inner =
      msg::CreateWorkerRes::create(builder, &msg::CreateWorkerResArgs { rid });
    serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(msg_inner.as_union_value()),
        inner_type: msg::Any::CreateWorkerRes,
        ..Default::default()
      },
    )
  };

  // Has provided source code, execute immediately.
  if has_source_code {
    worker.execute(&source_code).unwrap();
    return ok_buf(exec_cb(worker));
  }

  let op = worker
    .execute_mod_async(&module_specifier, false)
    .and_then(move |()| Ok(exec_cb(worker)));

  let result = op.wait()?;
  Ok(Op::Sync(result))
}

/// Return when the worker closes
pub fn op_host_get_worker_closed(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if base.sync() {
    return Err(deno_error::no_sync_support());
  }
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_get_worker_closed().unwrap();
  let rid = inner.rid();
  let state = state.clone();

  let shared_worker_future = {
    let workers_tl = state.workers.lock().unwrap();
    let worker = workers_tl.get(&rid).unwrap();
    worker.clone()
  };

  let op = Box::new(shared_worker_future.then(move |_result| {
    let builder = &mut FlatBufferBuilder::new();

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        ..Default::default()
      },
    ))
  }));
  Ok(Op::Async(Box::new(op)))
}

/// Get message from guest worker as host
pub fn op_host_get_message(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if base.sync() {
    return Err(deno_error::no_sync_support());
  }
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_get_message().unwrap();
  let rid = inner.rid();

  let op = resources::get_message_from_worker(rid);
  let op = op.map_err(move |_| -> ErrBox { unimplemented!() });
  let op = op.and_then(move |maybe_buf| -> Result<Buf, ErrBox> {
    let builder = &mut FlatBufferBuilder::new();

    let data = maybe_buf.as_ref().map(|buf| builder.create_vector(buf));
    let msg_inner = msg::HostGetMessageRes::create(
      builder,
      &msg::HostGetMessageResArgs { data },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(msg_inner.as_union_value()),
        inner_type: msg::Any::HostGetMessageRes,
        ..Default::default()
      },
    ))
  });
  Ok(Op::Async(Box::new(op)))
}

/// Post message to guest worker as host
pub fn op_host_post_message(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_post_message().unwrap();
  let rid = inner.rid();

  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  resources::post_message_to_worker(rid, d)
    .wait()
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;
  let builder = &mut FlatBufferBuilder::new();

  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      ..Default::default()
    },
  ))
}
