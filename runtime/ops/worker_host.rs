// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::poll_fn;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::DetachedBuffer;
use deno_core::FromV8;
use deno_core::JsBuffer;
use deno_core::JsRuntimeInspector;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_permissions::ChildPermissionsArg;
use deno_permissions::PermissionsContainer;
use deno_web::Blob;
use deno_web::BlobStoreTrait;
use deno_web::JsMessageData;
use deno_web::MessagePortError;
use deno_web::RecvMessageData;
use deno_web::Transferable;
use deno_web::deserialize_js_transferables;
use deno_web::serialize_transferables;
use log::debug;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

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

/// Default OS stack size for the thread a worker's isolate runs on.
///
/// This is `deno_node`'s `DEFAULT_STACK_SIZE_MB`, which is what
/// `resourceLimits.stackSizeMb` reports back to JS: without setting it here the
/// thread would get Rust's 2MB default while we told the user it was 4MB. 2MB
/// also leaves very little native headroom above V8's own JS stack limit
/// (`--stack-size`, 1MB by default), so a raised `--stack-size` overflows the
/// OS stack and aborts the process instead of raising `RangeError`.
const DEFAULT_WORKER_STACK_SIZE_MB: usize =
  deno_node::ops::worker_threads::DEFAULT_STACK_SIZE_MB;

/// V8 resource limits for worker isolates, matching Node.js `resourceLimits`.
#[derive(FromV8, Default, Clone)]
pub struct ResourceLimits {
  pub max_young_generation_size_mb: Option<usize>,
  pub max_old_generation_size_mb: Option<usize>,
  pub code_range_size_mb: Option<usize>,
  pub stack_size_mb: Option<usize>,
}

pub struct CreateWebWorkerArgs {
  pub name: String,
  pub worker_id: WorkerId,
  pub parent_permissions: PermissionsContainer,
  pub permissions: PermissionsContainer,
  pub main_module: ModuleSpecifier,
  pub worker_type: WorkerThreadType,
  pub close_on_idle: bool,
  pub maybe_worker_metadata: Option<WorkerMetadata>,
  /// Captured root blob for `main_module`; paired with `main_module` by the
  /// worker's loader/handle. Blob dependencies are intentionally resolved
  /// normally by their own URLs.
  pub maybe_main_module_blob: Option<Arc<Blob>>,
  pub resource_limits: Option<ResourceLimits>,
  pub wait_for_debugger_on_start: bool,
  pub wait_for_page_wait_for_debugger: bool,
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
  worker_type: WorkerThreadType,
  cancel_handle: Rc<CancelHandle>,
  cpu_thread_handle: Arc<AtomicU64>,
  web_lock_client_id: Option<String>,

  // A WorkerThread that hasn't been explicitly terminated can only be removed
  // from the WorkersTable once close messages have been received for both the
  // control and message channels. See `close_channel`.
  ctrl_closed: bool,
  message_closed: bool,
  termination_requested: bool,
}

impl WorkerThread {
  fn request_termination(&mut self) {
    self.termination_requested = true;
    self.worker_handle.clone().terminate();
  }

  fn finish_termination(self) {
    // Cancel recv ops when terminating the worker, so they don't show up as
    // pending ops.
    self.cancel_handle.cancel();
  }
}

impl Drop for WorkerThread {
  fn drop(&mut self) {
    // Promptly release the worker's Web Locks so other clients waiting on them
    // don't stay blocked until the worker thread fully unwinds. This is a
    // promptness optimization on top of the real backstop: the worker's held
    // and pending lock resources live in its own op_state and
    // release/cancel by id when its `JsRuntime` drops.
    //
    // `cleanup_locks_for_client_id` re-grants the worker's held locks to other
    // clients synchronously. If the worker were still executing a callback under
    // an exclusive lock (e.g. mutating a `SharedArrayBuffer` in a synchronous
    // loop), the new grantee could run concurrently with it, violating mutual
    // exclusion. `terminate()` alone doesn't prevent this: it only wakes the
    // event loop and can't interrupt synchronous JS already in flight. So when
    // the worker actually holds a lock we're about to hand off, we first call
    // `terminate_execution()`, which makes the worker's isolate throw a
    // termination exception at the next interrupt point, halting any such loop
    // and its callback continuation/microtasks before the lock is handed off.
    //
    // The `client_holds_lock` gate matters: `terminate_execution()` can abort an
    // in-progress synthetic module instantiation (e.g. a lazy `require` during
    // boot, which panics on failure), so we must not force-halt a worker that
    // has no held lock to protect. A worker that holds a lock is past boot and
    // parked in — or synchronously looping inside — its lock callback.
    //
    // This narrows the window but can't fully close it: `terminate_execution()`
    // returns without waiting for the isolate to stop, so a native op already in
    // flight on the worker keeps running until it returns to JS, and a lock
    // acquired between the `client_holds_lock` check and cleanup isn't halted.
    // Any lock left held in that residual window is still released by the
    // resource-drop backstop when the worker's `JsRuntime` drops.
    let handle = self.worker_handle.clone();
    if let Some(client_id) = &self.web_lock_client_id
      && deno_web::locks::client_holds_lock(client_id)
    {
      handle.terminate_execution();
    }
    handle.terminate();
    if let Some(client_id) = &self.web_lock_client_id {
      deno_web::locks::cleanup_locks_for_client_id(client_id);
    }
  }
}

pub type WorkersTable = HashMap<WorkerId, WorkerThread>;

// ============================================================
// Cross-thread messaging registry for `worker_threads.postMessageToThread`
// ============================================================
//
// Each Node-style worker thread (and the main thread) registers itself in
// this process-wide table so that any other thread can address it by id.
// The table holds the sender half of an mpsc channel plus a count of the
// destination thread's `workerMessage` event listeners; the receiver half
// lives in the thread's own resource table.

type ThreadMessage = (DetachedBuffer, Vec<Transferable>);

struct ThreadRegistryEntry {
  sender: UnboundedSender<ThreadMessage>,
  listener_count: AtomicU32,
}

static THREAD_REGISTRY: LazyLock<
  Mutex<HashMap<u32, Arc<ThreadRegistryEntry>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct ThreadMessageReceiver {
  thread_id: u32,
  rx: RefCell<UnboundedReceiver<ThreadMessage>>,
  cancel: CancelHandle,
}

impl Resource for ThreadMessageReceiver {
  fn name(&self) -> std::borrow::Cow<'_, str> {
    "threadMessageReceiver".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl Drop for ThreadMessageReceiver {
  fn drop(&mut self) {
    // Worker ids never repeat (they come from a monotonic counter and
    // main is always 0), and the JS side guards against double-register,
    // so the only entry for our thread_id is the one we installed.
    if let Ok(mut registry) = THREAD_REGISTRY.lock() {
      registry.remove(&self.thread_id);
    }
  }
}

deno_core::extension!(
  deno_worker_host,
  ops = [
    op_create_worker,
    op_host_terminate_worker,
    op_host_post_message,
    op_host_recv_ctrl,
    op_host_post_message_raw,
    op_host_recv_message,
    op_host_recv_message_sync,
    op_host_get_worker_cpu_usage,
    op_current_thread_cpu_usage,
    op_node_worker_thread_register,
    op_node_worker_thread_set_listener_count,
    op_node_worker_thread_post_message,
    op_node_worker_thread_recv_message,
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

#[derive(FromV8)]
pub struct CreateWorkerArgs {
  has_source_code: bool,
  name: Option<String>,
  #[from_v8(serde)]
  permissions: Option<ChildPermissionsArg>,
  source_code: String,
  specifier: String,
  #[from_v8(serde)]
  worker_type: WorkerThreadType,
  close_on_idle: bool,
  resource_limits: Option<ResourceLimits>,
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
fn op_create_worker(
  state: &mut OpState,
  #[scoped] args: CreateWorkerArgs,
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
  let wait_for_debugger_on_start = state
    .try_borrow::<Rc<JsRuntimeInspector>>()
    .map(|inspector| inspector.should_wait_for_debugger_on_worker_start())
    .unwrap_or(false);
  let wait_for_page_wait_for_debugger = state
    .try_borrow::<Rc<JsRuntimeInspector>>()
    .map(|inspector| {
      inspector.should_wait_for_page_wait_for_debugger_on_worker_start()
    })
    .unwrap_or(false);
  let worker_id = WorkerId::new();

  let module_specifier = deno_core::resolve_url(&specifier)?;
  // Synchronously capture the root blob so a racing `URL.revokeObjectURL`
  // after `new Worker(blobUrl)` can't make the worker load fail (see #26142).
  // This anchors only the worker root; blob URL dependencies still resolve
  // through the normal blob store at load time.
  let maybe_main_module_blob = if module_specifier.scheme() == "blob" {
    let blob_store = state.borrow::<Arc<dyn BlobStoreTrait>>();
    blob_store.get_object_url(module_specifier.clone())
  } else {
    None
  };
  let worker_name = args_name.unwrap_or_default();

  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<SendableWebWorkerHandle>(1);

  // Setup new thread. stackSizeMb from resourceLimits wins, matching Node.js
  // behavior; otherwise the isolate thread gets the default above rather than
  // Rust's smaller 2MB one.
  let stack_size_mb = args
    .resource_limits
    .as_ref()
    .and_then(|limits| limits.stack_size_mb.filter(|&v| v > 0))
    .unwrap_or(DEFAULT_WORKER_STACK_SIZE_MB);
  let thread_builder = std::thread::Builder::new()
    .name(format!("{worker_id}"))
    .stack_size(stack_size_mb * 1024 * 1024);
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
          maybe_main_module_blob,
          resource_limits: args.resource_limits,
          wait_for_debugger_on_start,
          wait_for_page_wait_for_debugger,
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

    let _ = create_and_run_current_thread(fut);

    // After the worker's tokio runtime and JsRuntime/V8 isolate have been
    // dropped, ask the system allocator to release freed memory back to the
    // OS. Without this, glibc in particular holds onto the fragmented heap
    // pages, causing RSS to remain high after many workers are created and
    // destroyed (https://github.com/denoland/deno/issues/26058).
    #[cfg(target_os = "linux")]
    {
      // SAFETY: calling libc function with no preconditions.
      unsafe {
        libc::malloc_trim(0);
      }
    }
  })?;

  // Receive WebWorkerHandle from newly created worker
  let worker_handle = handle_receiver.recv().map_err(|_| {
    std::io::Error::other(
      "Worker thread terminated unexpectedly before initialization completed",
    )
  })?;

  let worker_thread = WorkerThread {
    worker_handle: worker_handle.into(),
    worker_type: args.worker_type,
    cancel_handle: CancelHandle::new_rc(),
    cpu_thread_handle,
    // Only Node workers get a host-assigned `worker-N` lock client id (and thus
    // prompt cleanup on teardown). `web_worker.rs` assigns the matching id at
    // startup for the same worker types. Classic/module Web Workers keep the
    // default lazily-assigned numeric client id and rely on the resource-drop
    // backstop to release their locks when their `JsRuntime` unwinds.
    web_lock_client_id: matches!(args.worker_type, WorkerThreadType::Node)
      .then(|| deno_web::locks::worker_lock_client_id(worker_id.as_u32())),
    ctrl_closed: false,
    message_closed: false,
    termination_requested: false,
  };

  // At this point all interactions with worker happen using thread
  // safe handler returned from previous function calls
  state
    .borrow_mut::<WorkersTable>()
    .insert(worker_id, worker_thread);

  Ok(worker_id)
}

#[op2]
fn op_host_terminate_worker(state: &mut OpState, #[scoped] id: WorkerId) {
  match state.borrow_mut::<WorkersTable>().entry(id) {
    std::collections::hash_map::Entry::Occupied(mut entry) => {
      entry.get_mut().request_termination();
    }
    std::collections::hash_map::Entry::Vacant(_) => {
      debug!("tried to terminate non-existent worker {}", id);
    }
  }
}

enum WorkerChannel {
  Ctrl,
  Messages,
}

/// Close a worker's channel. If this results in a worker no longer needing
/// host-side receive ops, the worker will be removed from the workers table.
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
    let remove = {
      let worker_thread = entry.get_mut();
      match channel {
        WorkerChannel::Ctrl => {
          worker_thread.ctrl_closed = true;
          worker_thread.termination_requested || worker_thread.message_closed
        }
        WorkerChannel::Messages => {
          worker_thread.message_closed = true;
          !worker_thread.termination_requested && worker_thread.ctrl_closed
        }
      }
    };

    if remove {
      entry.remove().finish_termination();
    }
  }
}

/// Get control event from guest worker as host
#[op2]
#[serde]
async fn op_host_recv_ctrl(
  state: Rc<RefCell<OpState>>,
  #[scoped] id: WorkerId,
) -> WorkerControlEvent {
  let (worker_handle, cancel_handle) = {
    let state = state.borrow();
    let workers_table = state.borrow::<WorkersTable>();
    let maybe_handle = workers_table.get(&id);
    if let Some(handle) = maybe_handle {
      (handle.worker_handle.clone(), handle.cancel_handle.clone())
    } else {
      // If handle was not found it means worker has already shutdown
      return WorkerControlEvent::Close(0);
    }
  };

  let maybe_event = worker_handle
    .get_control_event()
    .or_cancel(cancel_handle)
    .await;
  match maybe_event {
    Ok(Some(event)) => {
      // Terminal error or close means that worker should be removed from worker table.
      match &event {
        WorkerControlEvent::TerminalError(..)
        | WorkerControlEvent::Close(_) => {
          close_channel(state, id, WorkerChannel::Ctrl);
        }
      }
      event
    }
    Ok(None) => {
      // If there was no event from worker it means it has already been closed.
      let exit_code = {
        let state = state.borrow();
        let workers_table = state.borrow::<WorkersTable>();
        workers_table
          .get(&id)
          .filter(|worker| {
            matches!(worker.worker_type, WorkerThreadType::Node)
              && worker.termination_requested
          })
          .map(|_| 1)
          .unwrap_or(0)
      };
      close_channel(state, id, WorkerChannel::Ctrl);
      WorkerControlEvent::Close(exit_code)
    }
    Err(_) => {
      // The worker was terminated.
      WorkerControlEvent::Close(0)
    }
  }
}

#[op2]
async fn op_host_recv_message(
  state: Rc<RefCell<OpState>>,
  #[scoped] id: WorkerId,
) -> Result<Option<RecvMessageData>, MessagePortError> {
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

#[op2]
fn op_host_recv_message_sync(
  state: &mut OpState,
  #[scoped] id: WorkerId,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let worker_handle = {
    let workers_table = state.borrow::<WorkersTable>();
    match workers_table.get(&id) {
      Some(handle) => handle.worker_handle.clone(),
      None => return Ok(None),
    }
  };
  worker_handle.port.try_recv_sync(state)
}

/// Post message to guest worker as host
#[op2]
fn op_host_post_message(
  state: &mut OpState,
  #[scoped] id: WorkerId,
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

/// Fast-path post: takes a pre-serialized buffer directly, bypassing
/// the JsMessageData serde overhead. Only for messages with no transferables.
#[op2]
fn op_host_post_message_raw(
  state: &mut OpState,
  #[scoped] id: WorkerId,
  #[buffer(detach)] data: JsBuffer,
) -> Result<(), MessagePortError> {
  if let Some(worker_thread) = state.borrow::<WorkersTable>().get(&id) {
    let worker_handle = worker_thread.worker_handle.clone();
    let detached = DetachedBuffer::from_v8slice(data.into_parts());
    if let Some(tx) = &*worker_handle.port.tx.borrow() {
      tx.send((detached, vec![])).ok();
    }
  }
  Ok(())
}

// ============================================================
// Cross-thread messaging ops (Node `worker_threads.postMessageToThread`)
// ============================================================

/// Register the current Node thread (main or worker) in the process-wide
/// thread registry so that other threads can `postMessageToThread` to it.
/// Returns the resource id of the receive-side channel, which the caller
/// is expected to keep alive for the lifetime of the thread and to poll
/// via `op_node_worker_thread_recv_message`.
#[op2(fast)]
#[smi]
fn op_node_worker_thread_register(
  state: &mut OpState,
  thread_id: u32,
) -> ResourceId {
  let (tx, rx) = unbounded_channel::<ThreadMessage>();
  let entry = Arc::new(ThreadRegistryEntry {
    sender: tx,
    listener_count: AtomicU32::new(0),
  });
  // Replace any pre-existing entry for this id (e.g. if a worker thread
  // is recycled in tests).
  THREAD_REGISTRY.lock().unwrap().insert(thread_id, entry);
  state.resource_table.add(ThreadMessageReceiver {
    thread_id,
    rx: RefCell::new(rx),
    cancel: CancelHandle::default(),
  })
}

/// Update the destination's `workerMessage` listener count. Posts to a
/// thread with zero listeners synchronously fail with
/// `ERR_WORKER_MESSAGING_FAILED`, matching Node.js semantics.
#[op2(fast)]
fn op_node_worker_thread_set_listener_count(thread_id: u32, count: u32) {
  let registry = THREAD_REGISTRY.lock().unwrap();
  if let Some(entry) = registry.get(&thread_id) {
    entry.listener_count.store(count, Ordering::SeqCst);
  }
}

/// Post a structured-clone payload to the thread identified by
/// `target_thread_id`.
///
/// Return value (kept as a small int for cheap FFI; mapped on the JS side):
///   0 — no thread with that id is currently registered.
///   1 — the destination thread has no `workerMessage` listeners.
///   2 — the message was enqueued for delivery.
///
/// If `force` is true the listener-count gate is skipped, which is used
/// for internal ack messages — the sender is, by construction, awaiting
/// the reply even though it has no public listener of its own.
#[op2]
fn op_node_worker_thread_post_message(
  state: &mut OpState,
  target_thread_id: u32,
  #[serde] data: JsMessageData,
  force: bool,
) -> Result<u8, MessagePortError> {
  let entry = match THREAD_REGISTRY.lock().unwrap().get(&target_thread_id) {
    Some(e) => e.clone(),
    None => return Ok(0),
  };
  if !force && entry.listener_count.load(Ordering::SeqCst) == 0 {
    return Ok(1);
  }
  let transferables = if data.transferables.is_empty() {
    vec![]
  } else {
    deserialize_js_transferables(state, data.transferables)?
  };
  if entry.sender.send((data.data, transferables)).is_err() {
    // Receiver was dropped between the registry lookup and the send.
    return Ok(0);
  }
  Ok(2)
}

/// Receive the next cross-thread message addressed to this thread.
/// Resolves to `None` when the channel is closed (i.e. the thread is
/// being torn down), at which point the JS-side poll loop terminates.
#[op2]
async fn op_node_worker_thread_recv_message(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let resource = {
    let state = state.borrow();
    match state.resource_table.get::<ThreadMessageReceiver>(rid) {
      Ok(r) => r,
      Err(_) => return Ok(None),
    }
  };
  let cancel = RcRef::map(resource.clone(), |r| &r.cancel);
  let recv = poll_fn(|cx| {
    let mut rx = resource.rx.borrow_mut();
    rx.poll_recv(cx)
  });
  let maybe_msg = match recv.or_cancel(cancel).await {
    Ok(m) => m,
    Err(_) => return Ok(None),
  };
  let (data, transferables) = match maybe_msg {
    Some(m) => m,
    None => return Ok(None),
  };
  let js_transferables = if transferables.is_empty() {
    vec![]
  } else {
    serialize_transferables(&mut state.borrow_mut(), transferables)
  };
  Ok(Some(JsMessageData {
    data,
    transferables: js_transferables,
  }))
}

#[op2]
fn op_host_get_worker_cpu_usage(
  state: &mut OpState,
  #[scoped] id: WorkerId,
  #[buffer] out: &mut [f64],
) {
  if out.len() < 2 {
    return;
  }

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

#[op2(fast)]
fn op_current_thread_cpu_usage(#[buffer] out: &mut [f64]) {
  if out.len() < 2 {
    return;
  }

  let handle = capture_current_thread_handle();
  let (user, system) = get_thread_cpu_usage_by_handle(handle);
  out[0] = user;
  out[1] = system;
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
  #[allow(clippy::disallowed_methods, reason = "requires real fs")]
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
  unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() as u64 }
}

#[cfg(windows)]
fn get_thread_cpu_usage_by_handle(handle: u64) -> (f64, f64) {
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::Foundation::FILETIME;
  use windows_sys::Win32::System::Threading::GetThreadTimes;
  use windows_sys::Win32::System::Threading::OpenThread;
  use windows_sys::Win32::System::Threading::THREAD_QUERY_INFORMATION;

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
