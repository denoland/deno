// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::ops::TestingFeaturesEnabled;
use crate::permissions::create_child_permissions;
use crate::permissions::ChildPermissionsArg;
use crate::permissions::Permissions;
use crate::web_worker::run_web_worker;
use crate::web_worker::SendableWebWorkerHandle;
use crate::web_worker::WebWorker;
use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WebWorkerType;
use crate::web_worker::WorkerControlEvent;
use crate::web_worker::WorkerId;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalFutureObj;
use deno_core::op;

use deno_core::serde::Deserialize;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_web::JsMessageData;
use log::debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct CreateWebWorkerArgs {
  pub name: String,
  pub worker_id: WorkerId,
  pub parent_permissions: Permissions,
  pub permissions: Permissions,
  pub main_module: ModuleSpecifier,
  pub use_deno_namespace: bool,
  pub worker_type: WebWorkerType,
  pub maybe_exit_code: Option<Arc<AtomicI32>>,
}

pub type CreateWebWorkerCb = dyn Fn(CreateWebWorkerArgs) -> (WebWorker, SendableWebWorkerHandle)
  + Sync
  + Send;

pub type PreloadModuleCb = dyn Fn(WebWorker) -> LocalFutureObj<'static, Result<WebWorker, AnyError>>
  + Sync
  + Send;

/// A holder for callback that is used to create a new
/// WebWorker. It's a struct instead of a type alias
/// because `GothamState` used in `OpState` overrides
/// value if type aliases have the same underlying type
#[derive(Clone)]
pub struct CreateWebWorkerCbHolder(Arc<CreateWebWorkerCb>);

/// A holder for callback that can used to preload some modules into a WebWorker
/// before actual worker code is executed. It's a struct instead of a type
/// because `GothamState` used in `OpState` overrides
/// value if type aliases have the same underlying type
#[derive(Clone)]
pub struct PreloadModuleCbHolder(Arc<PreloadModuleCb>);

pub struct WorkerThread {
  // It's an Option so we can take the value before dropping the WorkerThread.
  join_handle: Option<JoinHandle<Result<(), AnyError>>>,
  worker_handle: WebWorkerHandle,

  // A WorkerThread that hasn't been explicitly terminated can only be removed
  // from the WorkersTable once close messages have been received for both the
  // control and message channels. See `close_channel`.
  ctrl_closed: bool,
  message_closed: bool,
}

impl WorkerThread {
  fn terminate(mut self) {
    self.worker_handle.clone().terminate();
    self
      .join_handle
      .take()
      .unwrap()
      .join()
      .expect("Worker thread panicked")
      .expect("Panic in worker event loop");

    // Optimization so the Drop impl doesn't try to terminate the worker handle
    // again.
    self.ctrl_closed = true;
    self.message_closed = true;
  }
}

impl Drop for WorkerThread {
  fn drop(&mut self) {
    // If either of the channels is closed, the worker thread has at least
    // started closing, and its event loop won't start another run.
    if !(self.ctrl_closed || self.message_closed) {
      self.worker_handle.clone().terminate();
    }
  }
}

pub type WorkersTable = HashMap<WorkerId, WorkerThread>;

pub fn init(
  create_web_worker_cb: Arc<CreateWebWorkerCb>,
  preload_module_cb: Arc<PreloadModuleCb>,
) -> Extension {
  Extension::builder()
    .state(move |state| {
      state.put::<WorkersTable>(WorkersTable::default());
      state.put::<WorkerId>(WorkerId::default());

      let create_web_worker_cb_holder =
        CreateWebWorkerCbHolder(create_web_worker_cb.clone());
      state.put::<CreateWebWorkerCbHolder>(create_web_worker_cb_holder);
      let preload_module_cb_holder =
        PreloadModuleCbHolder(preload_module_cb.clone());
      state.put::<PreloadModuleCbHolder>(preload_module_cb_holder);

      Ok(())
    })
    .ops(vec![
      op_create_worker::decl(),
      op_host_terminate_worker::decl(),
      op_host_post_message::decl(),
      op_host_recv_ctrl::decl(),
      op_host_recv_message::decl(),
    ])
    .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
  has_source_code: bool,
  name: Option<String>,
  permissions: Option<ChildPermissionsArg>,
  source_code: String,
  specifier: String,
  use_deno_namespace: bool,
  worker_type: WebWorkerType,
}

/// Create worker as the host
#[op]
fn op_create_worker(
  state: &mut OpState,
  args: CreateWorkerArgs,
) -> Result<WorkerId, AnyError> {
  let specifier = args.specifier.clone();
  let maybe_source_code = if args.has_source_code {
    Some(args.source_code.clone())
  } else {
    None
  };
  let args_name = args.name;
  let use_deno_namespace = args.use_deno_namespace;
  if use_deno_namespace {
    super::check_unstable(state, "Worker.deno.namespace");
  }
  let worker_type = args.worker_type;
  if let WebWorkerType::Classic = worker_type {
    if let TestingFeaturesEnabled(false) = state.borrow() {
      return Err(
        deno_webstorage::DomExceptionNotSupportedError::new(
          "Classic workers are not supported.",
        )
        .into(),
      );
    }
  }

  if args.permissions.is_some() {
    super::check_unstable(state, "Worker.deno.permissions");
  }
  let parent_permissions = state.borrow_mut::<Permissions>();
  let worker_permissions = if let Some(child_permissions_arg) = args.permissions
  {
    create_child_permissions(parent_permissions, child_permissions_arg)?
  } else {
    parent_permissions.clone()
  };
  let parent_permissions = parent_permissions.clone();
  // `try_borrow` here, because worker might have been started without
  // access to `Deno` namespace.
  // TODO(bartlomieju): can a situation happen when parent doesn't
  // have access to `exit_code` but the child does?
  let maybe_exit_code = state.try_borrow::<Arc<AtomicI32>>().cloned();
  let worker_id = state.take::<WorkerId>();
  let create_web_worker_cb = state.take::<CreateWebWorkerCbHolder>();
  state.put::<CreateWebWorkerCbHolder>(create_web_worker_cb.clone());
  let preload_module_cb = state.take::<PreloadModuleCbHolder>();
  state.put::<PreloadModuleCbHolder>(preload_module_cb.clone());
  state.put::<WorkerId>(worker_id.next().unwrap());

  let module_specifier = deno_core::resolve_url(&specifier)?;
  let worker_name = args_name.unwrap_or_else(|| "".to_string());

  let (handle_sender, handle_receiver) = std::sync::mpsc::sync_channel::<
    Result<SendableWebWorkerHandle, AnyError>,
  >(1);

  // Setup new thread
  let thread_builder =
    std::thread::Builder::new().name(format!("{}", worker_id));

  // Spawn it
  let join_handle = thread_builder.spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits

    let (worker, external_handle) =
      (create_web_worker_cb.0)(CreateWebWorkerArgs {
        name: worker_name,
        worker_id,
        parent_permissions,
        permissions: worker_permissions,
        main_module: module_specifier.clone(),
        use_deno_namespace,
        worker_type,
        maybe_exit_code,
      });

    // Send thread safe handle from newly created worker to host thread
    handle_sender.send(Ok(external_handle)).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    run_web_worker(
      worker,
      module_specifier,
      maybe_source_code,
      preload_module_cb.0,
    )
  })?;

  // Receive WebWorkerHandle from newly created worker
  let worker_handle = handle_receiver.recv().unwrap()?;

  let worker_thread = WorkerThread {
    join_handle: Some(join_handle),
    worker_handle: worker_handle.into(),
    ctrl_closed: false,
    message_closed: false,
  };

  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function calls
  state
    .borrow_mut::<WorkersTable>()
    .insert(worker_id, worker_thread);

  Ok(worker_id)
}

#[op]
fn op_host_terminate_worker(
  state: &mut OpState,
  id: WorkerId,
) -> Result<(), AnyError> {
  if let Some(worker_thread) = state.borrow_mut::<WorkersTable>().remove(&id) {
    worker_thread.terminate();
  } else {
    debug!("tried to terminate non-existent worker {}", id);
  }
  Ok(())
}

enum WorkerChannel {
  Ctrl,
  Messages,
}

/// Close a worker's channel. If this results in both of a worker's channels
/// being closed, the worker will be removed from the workers table.
fn close_channel(
  state: Rc<RefCell<OpState>>,
  id: WorkerId,
  channel: WorkerChannel,
) {
  use std::collections::hash_map::Entry;

  let mut s = state.borrow_mut();
  let workers = s.borrow_mut::<WorkersTable>();

  // `Worker.terminate()` might have been called already, meaning that we won't
  // find the worker in the table - in that case ignore.
  if let Entry::Occupied(mut entry) = workers.entry(id) {
    let terminate = {
      let worker_thread = entry.get_mut();
      match channel {
        WorkerChannel::Ctrl => {
          worker_thread.ctrl_closed = true;
          worker_thread.message_closed
        }
        WorkerChannel::Messages => {
          worker_thread.message_closed = true;
          worker_thread.ctrl_closed
        }
      }
    };

    if terminate {
      entry.remove().terminate();
    }
  }
}

/// Get control event from guest worker as host
#[op]
async fn op_host_recv_ctrl(
  state: Rc<RefCell<OpState>>,
  id: WorkerId,
) -> Result<WorkerControlEvent, AnyError> {
  let worker_handle = {
    let state = state.borrow();
    let workers_table = state.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      handle.worker_handle.clone()
    } else {
      // If handle was not found it means worker has already shutdown
      return Ok(WorkerControlEvent::Close);
    }
  };

  let maybe_event = worker_handle.get_control_event().await?;
  if let Some(event) = maybe_event {
    // Terminal error means that worker should be removed from worker table.
    if let WorkerControlEvent::TerminalError(_) = &event {
      close_channel(state, id, WorkerChannel::Ctrl);
    }
    return Ok(event);
  }

  // If there was no event from worker it means it has already been closed.
  close_channel(state, id, WorkerChannel::Ctrl);
  Ok(WorkerControlEvent::Close)
}

#[op]
async fn op_host_recv_message(
  state: Rc<RefCell<OpState>>,
  id: WorkerId,
) -> Result<Option<JsMessageData>, AnyError> {
  let worker_handle = {
    let s = state.borrow();
    let workers_table = s.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      handle.worker_handle.clone()
    } else {
      // If handle was not found it means worker has already shutdown
      return Ok(None);
    }
  };

  let ret = worker_handle.port.recv(state.clone()).await?;
  if ret.is_none() {
    close_channel(state, id, WorkerChannel::Messages);
  }
  Ok(ret)
}

/// Post message to guest worker as host
#[op]
fn op_host_post_message(
  state: &mut OpState,
  id: WorkerId,
  data: JsMessageData,
) -> Result<(), AnyError> {
  if let Some(worker_thread) = state.borrow::<WorkersTable>().get(&id) {
    debug!("post message to worker {}", id);
    let worker_handle = worker_thread.worker_handle.clone();
    worker_handle.port.send(state, data)?;
  } else {
    debug!("tried to post message to non-existent worker {}", id);
  }
  Ok(())
}
