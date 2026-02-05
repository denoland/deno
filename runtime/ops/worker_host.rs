// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::op2;
use deno_core::serde::Deserialize;
use deno_permissions::ChildPermissionsArg;
use deno_permissions::PermissionsContainer;
use deno_web::JsMessageData;
use deno_web::MessagePortError;
use deno_web::deserialize_js_transferables;
use log::debug;

use crate::ops::TestingFeaturesEnabled;
use crate::tokio_util::create_and_run_current_thread;
use crate::web_worker::SendableWebWorkerHandle;
use crate::web_worker::WebWorker;
use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WorkerControlEvent;
use crate::web_worker::WorkerId;
use crate::web_worker::WorkerMetadata;
use crate::web_worker::WorkerThreadType;
use crate::web_worker::run_web_worker;
use crate::worker::FormatJsErrorFn;

pub const UNSTABLE_FEATURE_NAME: &str = "worker-options";

pub struct CreateWebWorkerArgs {
  pub name: String,
  pub worker_id: WorkerId,
  pub parent_permissions: PermissionsContainer,
  pub permissions: PermissionsContainer,
  pub main_module: ModuleSpecifier,
  pub worker_type: WorkerThreadType,
  pub close_on_idle: bool,
  pub maybe_worker_metadata: Option<WorkerMetadata>,
}

pub type CreateWebWorkerCb = dyn Fn(CreateWebWorkerArgs) -> (WebWorker, SendableWebWorkerHandle)
  + Sync
  + Send;

/// A holder for callback that is used to create a new
/// WebWorker. It's a struct instead of a type alias
/// because `GothamState` used in `OpState` overrides
/// value if type aliases have the same underlying type
#[derive(Clone)]
struct CreateWebWorkerCbHolder(Arc<CreateWebWorkerCb>);

#[derive(Clone)]
struct FormatJsErrorFnHolder(Option<Arc<FormatJsErrorFn>>);

pub struct WorkerThread {
  worker_handle: WebWorkerHandle,
  cancel_handle: Rc<CancelHandle>,
  cpu_thread_handle: Arc<AtomicU64>,

  // A WorkerThread that hasn't been explicitly terminated can only be removed
  // from the WorkersTable once close messages have been received for both the
  // control and message channels. See `close_channel`.
  ctrl_closed: bool,
  message_closed: bool,
}

impl WorkerThread {
  fn terminate(self) {
    // Cancel recv ops when terminating the worker, so they don't show up as
    // pending ops.
    self.cancel_handle.cancel();
  }
}

impl Drop for WorkerThread {
  fn drop(&mut self) {
    self.worker_handle.clone().terminate();
  }
}

pub type WorkersTable = HashMap<WorkerId, WorkerThread>;

deno_core::extension!(
  deno_worker_host,
  ops = [
    op_create_worker,
    op_host_terminate_worker,
    op_host_post_message,
    op_host_recv_ctrl,
    op_host_recv_message,
    op_host_get_worker_cpu_usage,
  ],
  options = {
    create_web_worker_cb: Arc<CreateWebWorkerCb>,
    format_js_error_fn: Option<Arc<FormatJsErrorFn>>,
  },
  state = |state, options| {
    state.put::<WorkersTable>(WorkersTable::default());

    let create_web_worker_cb_holder =
      CreateWebWorkerCbHolder(options.create_web_worker_cb);
    state.put::<CreateWebWorkerCbHolder>(create_web_worker_cb_holder);
    let format_js_error_fn_holder =
      FormatJsErrorFnHolder(options.format_js_error_fn);
    state.put::<FormatJsErrorFnHolder>(format_js_error_fn_holder);
  },
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
  has_source_code: bool,
  name: Option<String>,
  permissions: Option<ChildPermissionsArg>,
  source_code: String,
  specifier: String,
  worker_type: WorkerThreadType,
  close_on_idle: bool,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CreateWorkerError {
  #[class("DOMExceptionNotSupportedError")]
  #[error("Classic workers are not supported.")]
  ClassicWorkers,
  #[class(inherit)]
  #[error(transparent)]
  Permission(deno_permissions::ChildPermissionError),
  #[class(inherit)]
  #[error(transparent)]
  ModuleResolution(#[from] deno_core::ModuleResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  MessagePort(#[from] MessagePortError),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

/// Create worker as the host
#[op2(stack_trace)]
#[serde]
fn op_create_worker(
  state: &mut OpState,
  #[serde] args: CreateWorkerArgs,
  #[serde] maybe_worker_metadata: Option<JsMessageData>,
) -> Result<WorkerId, CreateWorkerError> {
  let specifier = args.specifier.clone();
  let maybe_source_code = if args.has_source_code {
    Some(args.source_code.clone())
  } else {
    None
  };
  let args_name = args.name;
  let worker_type = args.worker_type;
  if let WorkerThreadType::Classic = worker_type
    && let TestingFeaturesEnabled(false) = state.borrow()
  {
    return Err(CreateWorkerError::ClassicWorkers);
  }

  if args.permissions.is_some() {
    super::check_unstable(
      state,
      UNSTABLE_FEATURE_NAME,
      "Worker.deno.permissions",
    );
  }

  let parent_permissions = state.borrow_mut::<PermissionsContainer>();
  let worker_permissions = if let Some(child_permissions_arg) = args.permissions
  {
    parent_permissions
      .create_child_permissions(child_permissions_arg)
      .map_err(CreateWorkerError::Permission)?
  } else {
    parent_permissions.clone()
  };
  let parent_permissions = parent_permissions.clone();
  let create_web_worker_cb = state.borrow::<CreateWebWorkerCbHolder>().clone();
  let format_js_error_fn = state.borrow::<FormatJsErrorFnHolder>().clone();
  let worker_id = WorkerId::new();

  let module_specifier = deno_core::resolve_url(&specifier)?;
  let worker_name = args_name.unwrap_or_default();

  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<SendableWebWorkerHandle>(1);

  // Setup new thread
  let thread_builder = std::thread::Builder::new().name(format!("{worker_id}"));
  let maybe_worker_metadata = if let Some(data) = maybe_worker_metadata {
    let transferables =
      deserialize_js_transferables(state, data.transferables)?;
    Some(WorkerMetadata {
      buffer: data.data,
      transferables,
    })
  } else {
    None
  };
  let cpu_thread_handle = Arc::new(AtomicU64::new(0));
  let cpu_thread_handle_writer = cpu_thread_handle.clone();

  // Spawn it
  thread_builder.spawn(move || {
    // Capture the OS thread handle for CPU usage queries from the host.
    cpu_thread_handle_writer
      .store(capture_current_thread_handle(), Ordering::Release);

    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits
    let fut = async move {
      let (worker, external_handle) =
        (create_web_worker_cb.0)(CreateWebWorkerArgs {
          name: worker_name,
          worker_id,
          parent_permissions,
          permissions: worker_permissions,
          main_module: module_specifier.clone(),
          worker_type,
          close_on_idle: args.close_on_idle,
          maybe_worker_metadata,
        });

      // Send thread safe handle from newly created worker to host thread
      handle_sender.send(external_handle).unwrap();
      drop(handle_sender);

      // At this point the only method of communication with host
      // is using `worker.internal_channels`.
      //
      // Host can already push messages and interact with worker.
      run_web_worker(
        worker,
        module_specifier,
        maybe_source_code,
        format_js_error_fn.0,
      )
      .await
    };

    create_and_run_current_thread(fut)
  })?;

  // Receive WebWorkerHandle from newly created worker
  let worker_handle = handle_receiver.recv().unwrap();

  let worker_thread = WorkerThread {
    worker_handle: worker_handle.into(),
    cancel_handle: CancelHandle::new_rc(),
    cpu_thread_handle,
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

#[op2]
fn op_host_terminate_worker(state: &mut OpState, #[serde] id: WorkerId) {
  match state.borrow_mut::<WorkersTable>().remove(&id) {
    Some(worker_thread) => {
      worker_thread.terminate();
    }
    _ => {
      debug!("tried to terminate non-existent worker {}", id);
    }
  }
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
#[op2]
#[serde]
async fn op_host_recv_ctrl(
  state: Rc<RefCell<OpState>>,
  #[serde] id: WorkerId,
) -> WorkerControlEvent {
  let (worker_handle, cancel_handle) = {
    let state = state.borrow();
    let workers_table = state.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      (handle.worker_handle.clone(), handle.cancel_handle.clone())
    } else {
      // If handle was not found it means worker has already shutdown
      return WorkerControlEvent::Close;
    }
  };

  let maybe_event = worker_handle
    .get_control_event()
    .or_cancel(cancel_handle)
    .await;
  match maybe_event {
    Ok(Some(event)) => {
      // Terminal error means that worker should be removed from worker table.
      if let WorkerControlEvent::TerminalError(_) = &event {
        close_channel(state, id, WorkerChannel::Ctrl);
      }
      event
    }
    Ok(None) => {
      // If there was no event from worker it means it has already been closed.
      close_channel(state, id, WorkerChannel::Ctrl);
      WorkerControlEvent::Close
    }
    Err(_) => {
      // The worker was terminated.
      WorkerControlEvent::Close
    }
  }
}

#[op2]
#[serde]
async fn op_host_recv_message(
  state: Rc<RefCell<OpState>>,
  #[serde] id: WorkerId,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let (worker_handle, cancel_handle) = {
    let s = state.borrow();
    let workers_table = s.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      (handle.worker_handle.clone(), handle.cancel_handle.clone())
    } else {
      // If handle was not found it means worker has already shutdown
      return Ok(None);
    }
  };

  let ret = worker_handle
    .port
    .recv(state.clone())
    .or_cancel(cancel_handle)
    .await;
  match ret {
    Ok(Ok(ret)) => {
      if ret.is_none() {
        close_channel(state, id, WorkerChannel::Messages);
      }
      Ok(ret)
    }
    Ok(Err(err)) => Err(err),
    Err(_) => {
      // The worker was terminated.
      Ok(None)
    }
  }
}

/// Post message to guest worker as host
#[op2]
fn op_host_post_message(
  state: &mut OpState,
  #[serde] id: WorkerId,
  #[serde] data: JsMessageData,
) -> Result<(), MessagePortError> {
  if let Some(worker_thread) = state.borrow::<WorkersTable>().get(&id) {
    debug!("post message to worker {}", id);
    let worker_handle = worker_thread.worker_handle.clone();
    worker_handle.port.send(state, data)?;
  } else {
    debug!("tried to post message to non-existent worker {}", id);
  }
  Ok(())
}

#[op2]
fn op_host_get_worker_cpu_usage(
  state: &mut OpState,
  #[serde] id: WorkerId,
  #[buffer] out: &mut [f64],
) {
  if let Some(worker_thread) = state.borrow::<WorkersTable>().get(&id) {
    let handle = worker_thread.cpu_thread_handle.load(Ordering::Acquire);
    if handle != 0 {
      let (user, system) = get_thread_cpu_usage_by_handle(handle);
      out[0] = user;
      out[1] = system;
      return;
    }
  }
  out[0] = 0.0;
  out[1] = 0.0;
}

#[cfg(target_os = "macos")]
fn capture_current_thread_handle() -> u64 {
  // SAFETY: FFI call to get the current thread's Mach port.
  unsafe { mach_thread_self() as u64 }
}

#[cfg(target_os = "macos")]
fn get_thread_cpu_usage_by_handle(handle: u64) -> (f64, f64) {
  let thread_port = handle as u32;
  // SAFETY: thread_info() will initialize this
  let mut info: ThreadBasicInfo = unsafe { std::mem::zeroed() };
  let mut count: u32 = THREAD_BASIC_INFO_COUNT;

  // SAFETY: FFI call to query thread CPU times
  let kr = unsafe {
    thread_info(
      thread_port,
      THREAD_BASIC_INFO,
      (&raw mut info) as *mut i32,
      &mut count,
    )
  };

  if kr != 0 {
    return (0.0, 0.0);
  }

  let user =
    info.user_time_seconds as f64 * 1e6 + info.user_time_microseconds as f64;
  let system = info.system_time_seconds as f64 * 1e6
    + info.system_time_microseconds as f64;
  (user, system)
}

#[cfg(target_os = "macos")]
const THREAD_BASIC_INFO: u32 = 3;
#[cfg(target_os = "macos")]
const THREAD_BASIC_INFO_COUNT: u32 = 10;

#[cfg(target_os = "macos")]
#[repr(C)]
struct ThreadBasicInfo {
  user_time_seconds: i32,
  user_time_microseconds: i32,
  system_time_seconds: i32,
  system_time_microseconds: i32,
  cpu_usage: i32,
  policy: i32,
  run_state: i32,
  flags: i32,
  suspend_count: i32,
  sleep_time: i32,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
  fn mach_thread_self() -> u32;
  fn thread_info(
    target_act: u32,
    flavor: u32,
    thread_info_out: *mut i32,
    thread_info_outCnt: *mut u32,
  ) -> i32;
}

#[cfg(target_os = "linux")]
fn capture_current_thread_handle() -> u64 {
  // SAFETY: syscall to get the current thread ID.
  unsafe { libc::syscall(libc::SYS_gettid) as u64 }
}

#[cfg(target_os = "linux")]
fn get_thread_cpu_usage_by_handle(handle: u64) -> (f64, f64) {
  let tid = handle as i32;
  let path = format!("/proc/self/task/{}/stat", tid);
  #[allow(clippy::disallowed_methods)]
  if let Ok(contents) = std::fs::read_to_string(&path) {
    // Parse utime and stime after pid(comm)
    if let Some(pos) = contents.rfind(')') {
      let rest = &contents[pos + 2..]; // skip ") "
      let fields: Vec<&str> = rest.split_whitespace().collect();
      // 0=state 1=ppid ... 11=utime 12=stime
      if fields.len() > 12 {
        // SAFETY: sysconf call to get clock ticks per second.
        let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
        let utime = fields[11].parse::<f64>().unwrap_or(0.0);
        let stime = fields[12].parse::<f64>().unwrap_or(0.0);
        let user_us = utime / ticks_per_sec * 1e6;
        let system_us = stime / ticks_per_sec * 1e6;
        return (user_us, system_us);
      }
    }
  }
  (0.0, 0.0)
}

#[cfg(windows)]
fn capture_current_thread_handle() -> u64 {
  // SAFETY: Returns the thread ID of the calling thread.
  unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as u64 }
}

#[cfg(windows)]
fn get_thread_cpu_usage_by_handle(handle: u64) -> (f64, f64) {
  use winapi::shared::minwindef::FALSE;
  use winapi::shared::minwindef::FILETIME;
  use winapi::um::handleapi::CloseHandle;
  use winapi::um::processthreadsapi::GetThreadTimes;
  use winapi::um::processthreadsapi::OpenThread;
  use winapi::um::winnt::THREAD_QUERY_INFORMATION;

  let thread_id = handle as u32;

  // SAFETY: Opens a handle to the thread for querying times.
  let thread_handle =
    unsafe { OpenThread(THREAD_QUERY_INFORMATION, FALSE, thread_id) };
  if thread_handle.is_null() {
    return (0.0, 0.0);
  }

  let mut creation_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut exit_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut kernel_time = std::mem::MaybeUninit::<FILETIME>::uninit();
  let mut user_time = std::mem::MaybeUninit::<FILETIME>::uninit();

  // SAFETY: Queries thread CPU times.
  let ret = unsafe {
    GetThreadTimes(
      thread_handle,
      creation_time.as_mut_ptr(),
      exit_time.as_mut_ptr(),
      kernel_time.as_mut_ptr(),
      user_time.as_mut_ptr(),
    )
  };

  // SAFETY: Close the thread handle.
  unsafe { CloseHandle(thread_handle) };

  if ret == FALSE {
    return (0.0, 0.0);
  }

  // SAFETY: values are initialized.
  let user_time = unsafe { user_time.assume_init() };
  // SAFETY: values are initialized.
  let kernel_time = unsafe { kernel_time.assume_init() };

  // FILETIME is in 100-nanosecond intervals, convert to microseconds.
  let user_us = ((user_time.dwHighDateTime as u64) << 32
    | user_time.dwLowDateTime as u64) as f64
    / 10.0;
  let system_us = ((kernel_time.dwHighDateTime as u64) << 32
    | kernel_time.dwLowDateTime as u64) as f64
    / 10.0;
  (user_us, system_us)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn capture_current_thread_handle() -> u64 {
  0
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn get_thread_cpu_usage_by_handle(_handle: u64) -> (f64, f64) {
  (0.0, 0.0)
}
