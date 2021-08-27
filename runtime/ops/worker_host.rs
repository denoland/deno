// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ops::TestingFeaturesEnabled;
use crate::permissions::resolve_read_allowlist;
use crate::permissions::resolve_write_allowlist;
use crate::permissions::EnvDescriptor;
use crate::permissions::FfiDescriptor;
use crate::permissions::NetDescriptor;
use crate::permissions::PermissionState;
use crate::permissions::Permissions;
use crate::permissions::ReadDescriptor;
use crate::permissions::RunDescriptor;
use crate::permissions::UnaryPermission;
use crate::permissions::UnitPermission;
use crate::permissions::WriteDescriptor;
use crate::web_worker::run_web_worker;
use crate::web_worker::SendableWebWorkerHandle;
use crate::web_worker::WebWorker;
use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WebWorkerType;
use crate::web_worker::WorkerControlEvent;
use crate::web_worker::WorkerId;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serde::de;
use deno_core::serde::de::SeqAccess;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_web::JsMessageData;
use log::debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::From;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
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
}

pub type CreateWebWorkerCb = dyn Fn(CreateWebWorkerArgs) -> (WebWorker, SendableWebWorkerHandle)
  + Sync
  + Send;

/// A holder for callback that is used to create a new
/// WebWorker. It's a struct instead of a type alias
/// because `GothamState` used in `OpState` overrides
/// value if type alises have the same underlying type
#[derive(Clone)]
pub struct CreateWebWorkerCbHolder(Arc<CreateWebWorkerCb>);

pub struct WorkerThread {
  join_handle: JoinHandle<Result<(), AnyError>>,
  worker_handle: WebWorkerHandle,
}

pub type WorkersTable = HashMap<WorkerId, WorkerThread>;

pub fn init(create_web_worker_cb: Arc<CreateWebWorkerCb>) -> Extension {
  Extension::builder()
    .state(move |state| {
      state.put::<WorkersTable>(WorkersTable::default());
      state.put::<WorkerId>(WorkerId::default());

      let create_module_loader =
        CreateWebWorkerCbHolder(create_web_worker_cb.clone());
      state.put::<CreateWebWorkerCbHolder>(create_module_loader);

      Ok(())
    })
    .ops(vec![
      ("op_create_worker", op_sync(op_create_worker)),
      (
        "op_host_terminate_worker",
        op_sync(op_host_terminate_worker),
      ),
      ("op_host_post_message", op_sync(op_host_post_message)),
      ("op_host_recv_ctrl", op_async(op_host_recv_ctrl)),
      ("op_host_recv_message", op_async(op_host_recv_message)),
    ])
    .build()
}

fn merge_boolean_permission(
  mut main: UnitPermission,
  worker: Option<PermissionState>,
) -> Result<UnitPermission, AnyError> {
  if let Some(worker) = worker {
    if worker < main.state {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.state = worker;
    }
  }
  Ok(main)
}

fn merge_net_permission(
  mut main: UnaryPermission<NetDescriptor>,
  worker: Option<UnaryPermission<NetDescriptor>>,
) -> Result<UnaryPermission<NetDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker
        .granted_list
        .iter()
        .all(|x| main.check(&(&x.0, x.1)).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

fn merge_read_permission(
  mut main: UnaryPermission<ReadDescriptor>,
  worker: Option<UnaryPermission<ReadDescriptor>>,
) -> Result<UnaryPermission<ReadDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker
        .granted_list
        .iter()
        .all(|x| main.check(x.0.as_path()).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

fn merge_write_permission(
  mut main: UnaryPermission<WriteDescriptor>,
  worker: Option<UnaryPermission<WriteDescriptor>>,
) -> Result<UnaryPermission<WriteDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker
        .granted_list
        .iter()
        .all(|x| main.check(x.0.as_path()).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

fn merge_env_permission(
  mut main: UnaryPermission<EnvDescriptor>,
  worker: Option<UnaryPermission<EnvDescriptor>>,
) -> Result<UnaryPermission<EnvDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker.granted_list.iter().all(|x| main.check(&x.0).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

fn merge_run_permission(
  mut main: UnaryPermission<RunDescriptor>,
  worker: Option<UnaryPermission<RunDescriptor>>,
) -> Result<UnaryPermission<RunDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker.granted_list.iter().all(|x| main.check(&x.0).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

fn merge_ffi_permission(
  mut main: UnaryPermission<FfiDescriptor>,
  worker: Option<UnaryPermission<FfiDescriptor>>,
) -> Result<UnaryPermission<FfiDescriptor>, AnyError> {
  if let Some(worker) = worker {
    if (worker.global_state < main.global_state)
      || !worker.granted_list.iter().all(|x| main.check(&x.0).is_ok())
    {
      return Err(custom_error(
        "PermissionDenied",
        "Can't escalate parent thread permissions",
      ));
    } else {
      main.global_state = worker.global_state;
      main.granted_list = worker.granted_list;
    }
  }
  Ok(main)
}

pub fn create_worker_permissions(
  main_perms: Permissions,
  worker_perms: PermissionsArg,
) -> Result<Permissions, AnyError> {
  Ok(Permissions {
    env: merge_env_permission(main_perms.env, worker_perms.env)?,
    hrtime: merge_boolean_permission(main_perms.hrtime, worker_perms.hrtime)?,
    net: merge_net_permission(main_perms.net, worker_perms.net)?,
    ffi: merge_ffi_permission(main_perms.ffi, worker_perms.ffi)?,
    read: merge_read_permission(main_perms.read, worker_perms.read)?,
    run: merge_run_permission(main_perms.run, worker_perms.run)?,
    write: merge_write_permission(main_perms.write, worker_perms.write)?,
  })
}

#[derive(Debug, Deserialize)]
pub struct PermissionsArg {
  #[serde(default, deserialize_with = "as_unary_env_permission")]
  env: Option<UnaryPermission<EnvDescriptor>>,
  #[serde(default, deserialize_with = "as_permission_state")]
  hrtime: Option<PermissionState>,
  #[serde(default, deserialize_with = "as_unary_net_permission")]
  net: Option<UnaryPermission<NetDescriptor>>,
  #[serde(default, deserialize_with = "as_unary_ffi_permission")]
  ffi: Option<UnaryPermission<FfiDescriptor>>,
  #[serde(default, deserialize_with = "as_unary_read_permission")]
  read: Option<UnaryPermission<ReadDescriptor>>,
  #[serde(default, deserialize_with = "as_unary_run_permission")]
  run: Option<UnaryPermission<RunDescriptor>>,
  #[serde(default, deserialize_with = "as_unary_write_permission")]
  write: Option<UnaryPermission<WriteDescriptor>>,
}

fn as_permission_state<'de, D>(
  deserializer: D,
) -> Result<Option<PermissionState>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: bool = Deserialize::deserialize(deserializer)?;

  match value {
    true => Ok(Some(PermissionState::Granted)),
    false => Ok(Some(PermissionState::Denied)),
  }
}

struct UnaryPermissionBase {
  global_state: PermissionState,
  paths: Vec<String>,
}

struct ParseBooleanOrStringVec;

impl<'de> de::Visitor<'de> for ParseBooleanOrStringVec {
  type Value = UnaryPermissionBase;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a vector of strings or a boolean")
  }

  // visit_unit maps undefined/missing values to false
  fn visit_unit<E>(self) -> Result<UnaryPermissionBase, E>
  where
    E: de::Error,
  {
    self.visit_bool(false)
  }

  fn visit_bool<E>(self, v: bool) -> Result<UnaryPermissionBase, E>
  where
    E: de::Error,
  {
    Ok(UnaryPermissionBase {
      global_state: match v {
        true => PermissionState::Granted,
        false => PermissionState::Denied,
      },
      paths: Vec::new(),
    })
  }

  fn visit_seq<V>(self, mut visitor: V) -> Result<UnaryPermissionBase, V::Error>
  where
    V: SeqAccess<'de>,
  {
    let mut vec: Vec<String> = Vec::new();

    let mut value = visitor.next_element::<String>()?;
    while value.is_some() {
      vec.push(value.unwrap());
      value = visitor.next_element()?;
    }
    Ok(UnaryPermissionBase {
      global_state: PermissionState::Prompt,
      paths: vec,
    })
  }
}

fn as_unary_net_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<NetDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  let allowed: HashSet<NetDescriptor> = value
    .paths
    .into_iter()
    .map(NetDescriptor::from_string)
    .collect();

  Ok(Some(UnaryPermission::<NetDescriptor> {
    global_state: value.global_state,
    granted_list: allowed,
    ..Default::default()
  }))
}

fn as_unary_read_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<ReadDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  let paths: Vec<PathBuf> =
    value.paths.into_iter().map(PathBuf::from).collect();

  Ok(Some(UnaryPermission::<ReadDescriptor> {
    global_state: value.global_state,
    granted_list: resolve_read_allowlist(&Some(paths)),
    ..Default::default()
  }))
}

fn as_unary_write_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<WriteDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  let paths: Vec<PathBuf> =
    value.paths.into_iter().map(PathBuf::from).collect();

  Ok(Some(UnaryPermission::<WriteDescriptor> {
    global_state: value.global_state,
    granted_list: resolve_write_allowlist(&Some(paths)),
    ..Default::default()
  }))
}

fn as_unary_env_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<EnvDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  Ok(Some(UnaryPermission::<EnvDescriptor> {
    global_state: value.global_state,
    granted_list: value
      .paths
      .into_iter()
      .map(|env| EnvDescriptor(env.to_uppercase()))
      .collect(),
    ..Default::default()
  }))
}

fn as_unary_run_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<RunDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  Ok(Some(UnaryPermission::<RunDescriptor> {
    global_state: value.global_state,
    granted_list: value.paths.into_iter().map(RunDescriptor).collect(),
    ..Default::default()
  }))
}

fn as_unary_ffi_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<FfiDescriptor>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  Ok(Some(UnaryPermission::<FfiDescriptor> {
    global_state: value.global_state,
    granted_list: value.paths.into_iter().map(FfiDescriptor).collect(),
    ..Default::default()
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
  has_source_code: bool,
  name: Option<String>,
  permissions: Option<PermissionsArg>,
  source_code: String,
  specifier: String,
  use_deno_namespace: bool,
  worker_type: WebWorkerType,
}

/// Create worker as the host
fn op_create_worker(
  state: &mut OpState,
  args: CreateWorkerArgs,
  _: (),
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
  let parent_permissions = state.borrow::<Permissions>().clone();
  let worker_permissions = if let Some(permissions) = args.permissions {
    super::check_unstable(state, "Worker.deno.permissions");
    create_worker_permissions(parent_permissions.clone(), permissions)?
  } else {
    parent_permissions.clone()
  };

  let worker_id = state.take::<WorkerId>();
  let create_module_loader = state.take::<CreateWebWorkerCbHolder>();
  state.put::<CreateWebWorkerCbHolder>(create_module_loader.clone());
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
      (create_module_loader.0)(CreateWebWorkerArgs {
        name: worker_name,
        worker_id,
        parent_permissions,
        permissions: worker_permissions,
        main_module: module_specifier.clone(),
        use_deno_namespace,
        worker_type,
      });

    // Send thread safe handle from newly created worker to host thread
    handle_sender.send(Ok(external_handle)).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    run_web_worker(worker, module_specifier, maybe_source_code)
  })?;

  // Receive WebWorkerHandle from newly created worker
  let worker_handle = handle_receiver.recv().unwrap()?;

  let worker_thread = WorkerThread {
    join_handle,
    worker_handle: worker_handle.into(),
  };

  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function calls
  state
    .borrow_mut::<WorkersTable>()
    .insert(worker_id, worker_thread);

  Ok(worker_id)
}

fn op_host_terminate_worker(
  state: &mut OpState,
  id: WorkerId,
  _: (),
) -> Result<(), AnyError> {
  if let Some(worker_thread) = state.borrow_mut::<WorkersTable>().remove(&id) {
    worker_thread.worker_handle.terminate();
    worker_thread
      .join_handle
      .join()
      .expect("Panic in worker thread")
      .expect("Panic in worker event loop");
  } else {
    debug!("tried to terminate non-existent worker {}", id);
  }
  Ok(())
}

/// Try to remove worker from workers table - NOTE: `Worker.terminate()`
/// might have been called already meaning that we won't find worker in
/// table - in that case ignore.
fn try_remove_and_close(state: Rc<RefCell<OpState>>, id: WorkerId) {
  let mut s = state.borrow_mut();
  let workers = s.borrow_mut::<WorkersTable>();
  if let Some(worker_thread) = workers.remove(&id) {
    worker_thread.worker_handle.terminate();
    worker_thread
      .join_handle
      .join()
      .expect("Worker thread panicked")
      .expect("Panic in worker event loop");
  }
}

/// Get control event from guest worker as host
async fn op_host_recv_ctrl(
  state: Rc<RefCell<OpState>>,
  id: WorkerId,
  _: (),
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
      try_remove_and_close(state, id);
    }
    return Ok(event);
  }

  // If there was no event from worker it means it has already been closed.
  try_remove_and_close(state, id);
  Ok(WorkerControlEvent::Close)
}

async fn op_host_recv_message(
  state: Rc<RefCell<OpState>>,
  id: WorkerId,
  _: (),
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
  worker_handle.port.recv(state).await
}

/// Post message to guest worker as host
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
