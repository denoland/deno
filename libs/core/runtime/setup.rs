// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures::task::AtomicWaker;

use super::bindings;
use super::snapshot;
use super::snapshot::V8Snapshot;

/// Extract the raw isolate address from an `UnsafeRawIsolatePtr`.
///
/// `UnsafeRawIsolatePtr` is `#[repr(transparent)]` over `*mut RealIsolate`,
/// so its bit-pattern is a single pointer-sized value. We use transmute
/// because the inner field is private.
///
/// The compile-time assert below guarantees the layout assumption holds.
const _: () = assert!(
  std::mem::size_of::<v8::UnsafeRawIsolatePtr>()
    == std::mem::size_of::<usize>()
);

pub(crate) fn isolate_ptr_to_key(ptr: v8::UnsafeRawIsolatePtr) -> usize {
  // SAFETY: UnsafeRawIsolatePtr is #[repr(transparent)] over *mut RealIsolate,
  // which is pointer-sized. The compile-time assert above guarantees this.
  unsafe { std::mem::transmute::<v8::UnsafeRawIsolatePtr, usize>(ptr) }
}

/// Thread-safe queue of V8 foreground tasks, shared between the global
/// isolate registry (written by V8 background threads) and the event
/// loop (drained on the main thread). Cloning is cheap (Arc).
pub type ForegroundTaskQueue = std::sync::Arc<Mutex<Vec<v8::Task>>>;

/// Per-isolate state stored in the global registry. Kept minimal: just
/// enough for platform callbacks (which only have an isolate pointer) to
/// push tasks and wake the event loop.
struct IsolateEntry {
  waker: std::sync::Arc<AtomicWaker>,
  handle: tokio::runtime::Handle,
  tasks: ForegroundTaskQueue,
}

static ISOLATE_ENTRIES: std::sync::LazyLock<
  Mutex<HashMap<usize, IsolateEntry>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register an isolate in the global platform registry. The `tasks`
/// queue is shared with `JsRuntimeState` so the event loop drains it
/// directly without touching the global map.
pub fn register_isolate(
  isolate_ptr: usize,
  waker: std::sync::Arc<AtomicWaker>,
  handle: tokio::runtime::Handle,
  tasks: ForegroundTaskQueue,
) {
  let mut map = ISOLATE_ENTRIES.lock().unwrap();
  map.insert(
    isolate_ptr,
    IsolateEntry {
      waker,
      handle,
      tasks,
    },
  );
}

pub fn unregister_isolate(isolate_ptr: usize) {
  let mut map = ISOLATE_ENTRIES.lock().unwrap();
  map.remove(&isolate_ptr);
}

/// Queue an immediate foreground task and wake the event loop.
fn queue_task(key: usize, task: v8::Task) {
  let map = ISOLATE_ENTRIES.lock().unwrap();
  if let Some(entry) = map.get(&key) {
    entry.tasks.lock().unwrap().push(task);
    entry.waker.wake();
  }
}

/// Spawn a delayed V8 foreground task on the isolate's tokio runtime.
/// After the delay, the task is queued for synchronous draining (not
/// run directly on the tokio worker thread).
fn spawn_delayed_task(key: usize, task: v8::Task, delay_in_seconds: f64) {
  let map = ISOLATE_ENTRIES.lock().unwrap();
  if let Some(entry) = map.get(&key) {
    let tasks = entry.tasks.clone();
    let waker = entry.waker.clone();
    entry.handle.spawn(async move {
      tokio::time::sleep(Duration::from_secs_f64(delay_in_seconds)).await;
      tasks.lock().unwrap().push(task);
      waker.wake();
    });
  }
}

/// Custom V8 platform implementation that queues immediate foreground
/// tasks for synchronous draining, and spawns delayed tasks on tokio.
struct DenoPlatformImpl;

impl v8::PlatformImpl for DenoPlatformImpl {
  fn post_task(&self, isolate_ptr: *mut c_void, task: v8::Task) {
    queue_task(isolate_ptr as usize, task);
  }

  fn post_non_nestable_task(&self, isolate_ptr: *mut c_void, task: v8::Task) {
    queue_task(isolate_ptr as usize, task);
  }

  fn post_delayed_task(
    &self,
    isolate_ptr: *mut c_void,
    task: v8::Task,
    delay_in_seconds: f64,
  ) {
    spawn_delayed_task(isolate_ptr as usize, task, delay_in_seconds);
  }

  fn post_non_nestable_delayed_task(
    &self,
    isolate_ptr: *mut c_void,
    task: v8::Task,
    delay_in_seconds: f64,
  ) {
    spawn_delayed_task(isolate_ptr as usize, task, delay_in_seconds);
  }

  fn post_idle_task(&self, _isolate_ptr: *mut c_void, _task: v8::IdleTask) {
    unreachable!();
  }
}

fn v8_init(
  v8_platform: Option<v8::SharedRef<v8::Platform>>,
  snapshot: bool,
  expose_natives: bool,
) {
  #[cfg(feature = "include_icu_data")]
  {
    v8::icu::set_common_data_77(deno_core_icudata::ICU_DATA).unwrap();
  }

  let base_flags = concat!(
    " --wasm-test-streaming",
    " --no-validate-asm",
    " --turbo_fast_api_calls",
    " --harmony-temporal",
    " --js-float16array",
    " --js-explicit-resource-management",
    " --js-source-phase-imports"
  );
  let snapshot_flags = "--predictable --random-seed=42";
  let expose_natives_flags = "--expose_gc --allow_natives_syntax";
  let lazy_flags = if cfg!(feature = "snapshot_flags_eager_parse") {
    "--no-lazy --no-lazy-eval --no-lazy-streaming"
  } else {
    ""
  };
  let flags = match (snapshot, expose_natives) {
    (false, false) => base_flags.to_string(),
    (true, false) => {
      format!("{base_flags} {snapshot_flags} {lazy_flags}")
    }
    (false, true) => format!("{base_flags} {expose_natives_flags}"),
    (true, true) => {
      format!(
        "{base_flags} {snapshot_flags} {lazy_flags} {expose_natives_flags}"
      )
    }
  };
  v8::V8::set_flags_from_string(&flags);

  let v8_platform = v8_platform.unwrap_or_else(|| {
    let unprotected =
      cfg!(any(test, feature = "unsafe_use_unprotected_platform"));
    v8::new_custom_platform(0, false, unprotected, DenoPlatformImpl)
      .make_shared()
  });
  v8::V8::initialize_platform(v8_platform.clone());
  v8::V8::initialize();
}

pub fn init_v8(
  v8_platform: Option<v8::SharedRef<v8::Platform>>,
  snapshot: bool,
  expose_natives: bool,
) {
  static DENO_INIT: Once = Once::new();
  static DENO_SNAPSHOT: AtomicBool = AtomicBool::new(false);
  static DENO_SNAPSHOT_SET: AtomicBool = AtomicBool::new(false);

  if DENO_SNAPSHOT_SET.load(Ordering::SeqCst) {
    let current = DENO_SNAPSHOT.load(Ordering::SeqCst);
    assert_eq!(
      current, snapshot,
      "V8 may only be initialized once in either snapshotting or non-snapshotting mode. Either snapshotting or non-snapshotting mode may be used in a single process, not both."
    );
    DENO_SNAPSHOT_SET.store(true, Ordering::SeqCst);
    DENO_SNAPSHOT.store(snapshot, Ordering::SeqCst);
  }

  DENO_INIT.call_once(move || v8_init(v8_platform, snapshot, expose_natives));
}

pub fn create_isolate(
  will_snapshot: bool,
  maybe_create_params: Option<v8::CreateParams>,
  maybe_startup_snapshot: Option<V8Snapshot>,
  external_refs: Cow<'static, [v8::ExternalReference]>,
) -> v8::OwnedIsolate {
  let mut params = maybe_create_params.unwrap_or_default();
  let mut isolate = if will_snapshot {
    snapshot::create_snapshot_creator(
      external_refs,
      maybe_startup_snapshot,
      params,
    )
  } else {
    params = params.external_references(external_refs);
    let has_snapshot = maybe_startup_snapshot.is_some();
    if let Some(snapshot) = maybe_startup_snapshot {
      params = params.snapshot_blob(v8::StartupData::from(snapshot.0));
    }
    static FIRST_SNAPSHOT_INIT: AtomicBool = AtomicBool::new(false);
    static SNAPSHOW_INIT_MUT: Mutex<()> = Mutex::new(());

    // On Windows, the snapshot deserialization code appears to be crashing and we are not
    // certain of the reason. We take a mutex the first time an isolate with a snapshot to
    // prevent this. https://github.com/denoland/deno/issues/15590
    if cfg!(windows)
      && has_snapshot
      && FIRST_SNAPSHOT_INIT.load(Ordering::SeqCst)
    {
      let _g = SNAPSHOW_INIT_MUT.lock().unwrap();
      let res = v8::Isolate::new(params);
      FIRST_SNAPSHOT_INIT.store(true, Ordering::SeqCst);
      res
    } else {
      v8::Isolate::new(params)
    }
  };

  isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
  isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 10);
  isolate.set_promise_reject_callback(bindings::promise_reject_callback);
  isolate.set_prepare_stack_trace_callback(
    crate::error::prepare_stack_trace_callback,
  );
  isolate.set_host_initialize_import_meta_object_callback(
    bindings::host_initialize_import_meta_object_callback,
  );
  isolate.set_host_import_module_dynamically_callback(
    bindings::host_import_module_dynamically_callback,
  );
  isolate.set_host_import_module_with_phase_dynamically_callback(
    bindings::host_import_module_with_phase_dynamically_callback,
  );
  isolate.set_wasm_async_resolve_promise_callback(
    bindings::wasm_async_resolve_promise_callback,
  );

  isolate
}
