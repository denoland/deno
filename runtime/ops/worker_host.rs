// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::permissions::{
  resolve_fs_allowlist, PermissionState, Permissions, UnaryPermission,
};
use crate::web_worker::{
  run_web_worker, WebWorker, WebWorkerHandle, WorkerEvent,
};

use deno_core::error::{custom_error, generic_error, AnyError, JsError};
use deno_core::futures::channel::mpsc;
use deno_core::serde::de::{self, SeqAccess};
use deno_core::serde::{Deserialize, Deserializer};
use deno_core::serde_json::{self, json, Value};
use deno_core::{BufVec, ModuleSpecifier, OpState, ZeroCopyBuf};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct CreateWebWorkerArgs {
  pub name: String,
  pub worker_id: u32,
  pub parent_permissions: Permissions,
  pub permissions: Permissions,
  pub main_module: ModuleSpecifier,
  pub use_deno_namespace: bool,
}

pub type CreateWebWorkerCb =
  dyn Fn(CreateWebWorkerArgs) -> WebWorker + Sync + Send;

/// A holder for callback that is used to create a new
/// WebWorker. It's a struct instead of a type alias
/// because `GothamState` used in `OpState` overrides
/// value if type alises have the same underlying type
#[derive(Clone)]
pub struct CreateWebWorkerCbHolder(Arc<CreateWebWorkerCb>);

#[derive(Deserialize)]
struct HostUnhandledErrorArgs {
  message: String,
}

pub struct WorkerThread {
  join_handle: JoinHandle<Result<(), AnyError>>,
  worker_handle: WebWorkerHandle,
}

pub type WorkersTable = HashMap<u32, WorkerThread>;
pub type WorkerId = u32;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  sender: Option<mpsc::Sender<WorkerEvent>>,
  create_web_worker_cb: Arc<CreateWebWorkerCb>,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<WorkersTable>(WorkersTable::default());
    state.put::<WorkerId>(WorkerId::default());

    let create_module_loader = CreateWebWorkerCbHolder(create_web_worker_cb);
    state.put::<CreateWebWorkerCbHolder>(create_module_loader);
  }
  super::reg_json_sync(rt, "op_create_worker", op_create_worker);
  super::reg_json_sync(
    rt,
    "op_host_terminate_worker",
    op_host_terminate_worker,
  );
  super::reg_json_sync(rt, "op_host_post_message", op_host_post_message);
  super::reg_json_async(rt, "op_host_get_message", op_host_get_message);
  super::reg_json_sync(
    rt,
    "op_host_unhandled_error",
    move |_state, args, _zero_copy| {
      if let Some(mut sender) = sender.clone() {
        let args: HostUnhandledErrorArgs = serde_json::from_value(args)?;
        sender
          .try_send(WorkerEvent::Error(generic_error(args.message)))
          .expect("Failed to propagate error event to parent worker");
        Ok(json!(true))
      } else {
        Err(generic_error("Cannot be called from main worker."))
      }
    },
  );
}

fn merge_permission_state(
  target: &PermissionState,
  incoming: Option<PermissionState>,
) -> Result<PermissionState, AnyError> {
  match target {
    PermissionState::Granted => match incoming {
      Some(x) => Ok(x),
      None => Ok(*target),
    },
    _ => match incoming {
      Some(x) => match x {
        PermissionState::Denied => Ok(x),
        _ => Err(custom_error(
          "PermissionDenied",
          "Can't extend current permissions",
        )),
      },
      None => Ok(*target),
    },
  }
}

fn check_net_permission_contains(
  a: &HashSet<String>,
  b: &HashSet<String>,
) -> bool {
  b.iter().all(|x| a.contains(x))
}

fn merge_net_permissions(
  target: &UnaryPermission<String>,
  incoming: Option<UnaryPermission<String>>,
) -> Result<UnaryPermission<String>, AnyError> {
  // Default: use main thread permissions
  if incoming.is_none() {
    return Ok(target.clone());
  };

  let new_permissions = incoming.unwrap();
  match &target.global_state {
    PermissionState::Granted => Ok(UnaryPermission::<String> {
      global_state: new_permissions.global_state,
      granted_list: new_permissions.granted_list,
      denied_list: new_permissions.denied_list,
    }),
    PermissionState::Prompt => match new_permissions.global_state {
      //Throw
      PermissionState::Granted => Err(custom_error(
        "PermissionDenied",
        "Can't extend current permissions",
      )),
      //Merge
      PermissionState::Prompt => {
        if check_net_permission_contains(
          &target.granted_list,
          &new_permissions.granted_list,
        ) {
          Ok(UnaryPermission::<String> {
            global_state: new_permissions.global_state,
            granted_list: new_permissions.granted_list,
            denied_list: target.denied_list.clone(),
          })
        } else {
          Err(custom_error(
            "PermissionDenied",
            "Can't extend current permissions",
          ))
        }
      }
      //Copy
      PermissionState::Denied => Ok(UnaryPermission::<String> {
        global_state: new_permissions.global_state,
        granted_list: new_permissions.granted_list,
        denied_list: new_permissions.denied_list,
      }),
    },
    PermissionState::Denied => match new_permissions.global_state {
      PermissionState::Denied => Ok(UnaryPermission::<String> {
        global_state: new_permissions.global_state,
        granted_list: new_permissions.granted_list,
        denied_list: new_permissions.denied_list,
      }),
      _ => Err(custom_error(
        "PermissionDenied",
        "Can't extend current permissions",
      )),
    },
  }
}

enum WorkerPermissionType {
  READ,
  WRITE,
}

fn check_read_permissions(
  allow_list: &HashSet<PathBuf>,
  current_permissions: &Permissions,
) -> bool {
  allow_list
    .iter()
    .all(|x| current_permissions.check_read(&x).is_ok())
}

fn check_write_permissions(
  allow_list: &HashSet<PathBuf>,
  current_permissions: &Permissions,
) -> bool {
  allow_list
    .iter()
    .all(|x| current_permissions.check_write(&x).is_ok())
}

fn merge_unary_permissions(
  permission_type: WorkerPermissionType,
  target: &UnaryPermission<PathBuf>,
  incoming: Option<UnaryPermission<PathBuf>>,
  current_permissions: &Permissions,
) -> Result<UnaryPermission<PathBuf>, AnyError> {
  // Default: use main thread permissions
  if incoming.is_none() {
    return Ok(target.clone());
  };

  let new_permissions = incoming.unwrap();
  match &target.global_state {
    PermissionState::Granted => Ok(UnaryPermission::<PathBuf> {
      global_state: new_permissions.global_state,
      granted_list: new_permissions.granted_list,
      denied_list: new_permissions.denied_list,
    }),
    PermissionState::Prompt => match new_permissions.global_state {
      //Throw
      PermissionState::Granted => Err(custom_error(
        "PermissionDenied",
        "Can't extend current permissions",
      )),
      //Merge
      PermissionState::Prompt => {
        if match permission_type {
          WorkerPermissionType::READ => check_read_permissions(
            &new_permissions.granted_list,
            current_permissions,
          ),
          WorkerPermissionType::WRITE => check_write_permissions(
            &new_permissions.granted_list,
            current_permissions,
          ),
        } {
          Ok(UnaryPermission::<PathBuf> {
            global_state: new_permissions.global_state,
            granted_list: new_permissions.granted_list,
            denied_list: target.denied_list.clone(),
          })
        } else {
          Err(custom_error(
            "PermissionDenied",
            "Can't extend current permissions",
          ))
        }
      }
      //Copy
      PermissionState::Denied => Ok(UnaryPermission::<PathBuf> {
        global_state: new_permissions.global_state,
        granted_list: new_permissions.granted_list,
        denied_list: new_permissions.denied_list,
      }),
    },
    PermissionState::Denied => match new_permissions.global_state {
      PermissionState::Denied => Ok(UnaryPermission::<PathBuf> {
        global_state: new_permissions.global_state,
        granted_list: new_permissions.granted_list,
        denied_list: new_permissions.denied_list,
      }),
      _ => Err(custom_error(
        "PermissionDenied",
        "Can't extend current permissions",
      )),
    },
  }
}

fn create_worker_permissions(
  main_thread_permissions: &Permissions,
  permission_args: PermissionsArg,
) -> Result<Permissions, AnyError> {
  Ok(Permissions {
    env: merge_permission_state(
      &main_thread_permissions.env,
      permission_args.env,
    )?,
    hrtime: merge_permission_state(
      &main_thread_permissions.hrtime,
      permission_args.hrtime,
    )?,
    net: merge_net_permissions(
      &main_thread_permissions.net,
      permission_args.net,
    )?,
    plugin: merge_permission_state(
      &main_thread_permissions.plugin,
      permission_args.plugin,
    )?,
    read: merge_unary_permissions(
      WorkerPermissionType::READ,
      &main_thread_permissions.read,
      permission_args.read,
      &main_thread_permissions,
    )?,
    run: merge_permission_state(
      &main_thread_permissions.run,
      permission_args.run,
    )?,
    write: merge_unary_permissions(
      WorkerPermissionType::WRITE,
      &main_thread_permissions.write,
      permission_args.write,
      &main_thread_permissions,
    )?,
  })
}

#[derive(Debug, Deserialize)]
struct PermissionsArg {
  #[serde(default, deserialize_with = "as_permission_state")]
  env: Option<PermissionState>,
  #[serde(default, deserialize_with = "as_permission_state")]
  hrtime: Option<PermissionState>,
  #[serde(default, deserialize_with = "as_unary_string_permission")]
  net: Option<UnaryPermission<String>>,
  #[serde(default, deserialize_with = "as_permission_state")]
  plugin: Option<PermissionState>,
  #[serde(default, deserialize_with = "as_unary_path_permission")]
  read: Option<UnaryPermission<PathBuf>>,
  #[serde(default, deserialize_with = "as_permission_state")]
  run: Option<PermissionState>,
  #[serde(default, deserialize_with = "as_unary_path_permission")]
  write: Option<UnaryPermission<PathBuf>>,
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

fn as_unary_string_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<String>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  let allowed: HashSet<String> = value.paths.into_iter().collect();

  Ok(Some(UnaryPermission::<String> {
    global_state: value.global_state,
    granted_list: allowed,
    ..Default::default()
  }))
}

fn as_unary_path_permission<'de, D>(
  deserializer: D,
) -> Result<Option<UnaryPermission<PathBuf>>, D::Error>
where
  D: Deserializer<'de>,
{
  let value: UnaryPermissionBase =
    deserializer.deserialize_any(ParseBooleanOrStringVec)?;

  let paths: Vec<PathBuf> =
    value.paths.into_iter().map(PathBuf::from).collect();

  Ok(Some(UnaryPermission::<PathBuf> {
    global_state: value.global_state,
    granted_list: resolve_fs_allowlist(&paths),
    ..Default::default()
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWorkerArgs {
  has_source_code: bool,
  name: Option<String>,
  permissions: PermissionsArg,
  source_code: String,
  specifier: String,
  use_deno_namespace: bool,
}

/// Create worker as the host
fn op_create_worker(
  state: &mut OpState,
  args: Value,
  _data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateWorkerArgs = serde_json::from_value(args)?;

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
  //TODO(Soremwar)
  //Add unstable check for permissions
  let parent_permissions = state.borrow::<Permissions>().clone();
  let worker_permissions =
    create_worker_permissions(&parent_permissions, args.permissions)?;
  let worker_id = state.take::<WorkerId>();
  let create_module_loader = state.take::<CreateWebWorkerCbHolder>();
  state.put::<CreateWebWorkerCbHolder>(create_module_loader.clone());
  state.put::<WorkerId>(worker_id + 1);

  let module_specifier = ModuleSpecifier::resolve_url(&specifier)?;
  let worker_name = args_name.unwrap_or_else(|| "".to_string());

  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WebWorkerHandle, AnyError>>(1);

  // Setup new thread
  let thread_builder =
    std::thread::Builder::new().name(format!("deno-worker-{}", worker_id));

  // Spawn it
  let join_handle = thread_builder.spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits

    let worker = (create_module_loader.0)(CreateWebWorkerArgs {
      name: worker_name,
      worker_id,
      parent_permissions,
      permissions: worker_permissions,
      main_module: module_specifier.clone(),
      use_deno_namespace,
    });

    // Send thread safe handle to newly created worker to host thread
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    run_web_worker(worker, module_specifier, maybe_source_code)
  })?;

  let worker_handle = handle_receiver.recv().unwrap()?;

  let worker_thread = WorkerThread {
    join_handle,
    worker_handle,
  };

  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function calls
  state
    .borrow_mut::<WorkersTable>()
    .insert(worker_id, worker_thread);

  Ok(json!({ "id": worker_id }))
}

#[derive(Deserialize)]
struct WorkerArgs {
  id: i32,
}

fn op_host_terminate_worker(
  state: &mut OpState,
  args: Value,
  _data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let worker_thread = state
    .borrow_mut::<WorkersTable>()
    .remove(&id)
    .expect("No worker handle found");
  worker_thread.worker_handle.terminate();
  worker_thread
    .join_handle
    .join()
    .expect("Panic in worker thread")
    .expect("Panic in worker event loop");
  Ok(json!({}))
}

fn serialize_worker_event(event: WorkerEvent) -> Value {
  match event {
    WorkerEvent::Message(buf) => json!({ "type": "msg", "data": buf }),
    WorkerEvent::TerminalError(error) => match error.downcast::<JsError>() {
      Ok(js_error) => json!({
        "type": "terminalError",
        "error": {
          "message": js_error.message,
          "fileName": js_error.script_resource_name,
          "lineNumber": js_error.line_number,
          "columnNumber": js_error.start_column,
        }
      }),
      Err(error) => json!({
        "type": "terminalError",
        "error": {
          "message": error.to_string(),
        }
      }),
    },
    WorkerEvent::Error(error) => match error.downcast::<JsError>() {
      Ok(js_error) => json!({
        "type": "error",
        "error": {
          "message": js_error.message,
          "fileName": js_error.script_resource_name,
          "lineNumber": js_error.line_number,
          "columnNumber": js_error.start_column,
        }
      }),
      Err(error) => json!({
        "type": "error",
        "error": {
          "message": error.to_string(),
        }
      }),
    },
  }
}

/// Try to remove worker from workers table - NOTE: `Worker.terminate()`
/// might have been called already meaning that we won't find worker in
/// table - in that case ignore.
fn try_remove_and_close(state: Rc<RefCell<OpState>>, id: u32) {
  let mut s = state.borrow_mut();
  let workers = s.borrow_mut::<WorkersTable>();
  if let Some(mut worker_thread) = workers.remove(&id) {
    worker_thread.worker_handle.sender.close_channel();
    worker_thread
      .join_handle
      .join()
      .expect("Worker thread panicked")
      .expect("Panic in worker event loop");
  }
}

/// Get message from guest worker as host
async fn op_host_get_message(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;

  let worker_handle = {
    let s = state.borrow();
    let workers_table = s.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      handle.worker_handle.clone()
    } else {
      // If handle was not found it means worker has already shutdown
      return Ok(json!({ "type": "close" }));
    }
  };

  let maybe_event = worker_handle.get_event().await?;
  if let Some(event) = maybe_event {
    // Terminal error means that worker should be removed from worker table.
    if let WorkerEvent::TerminalError(_) = &event {
      try_remove_and_close(state, id);
    }
    return Ok(serialize_worker_event(event));
  }

  // If there was no event from worker it means it has already been closed.
  try_remove_and_close(state, id);
  Ok(json!({ "type": "close" }))
}

/// Post message to guest worker as host
fn op_host_post_message(
  state: &mut OpState,
  args: Value,
  data: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  assert_eq!(data.len(), 1, "Invalid number of arguments");
  let args: WorkerArgs = serde_json::from_value(args)?;
  let id = args.id as u32;
  let msg = Vec::from(&*data[0]).into_boxed_slice();

  debug!("post message to worker {}", id);
  let worker_thread = state
    .borrow::<WorkersTable>()
    .get(&id)
    .expect("No worker handle found");
  worker_thread.worker_handle.post_message(msg)?;
  Ok(json!({}))
}
