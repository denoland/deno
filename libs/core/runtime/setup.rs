// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

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

/// Per-isolate state shared between the V8 platform callback and
/// the event loop.
struct IsolateWakeEntry {
  waker: std::sync::Arc<AtomicWaker>,
}

/// Global registry mapping isolate pointers to their event loop wake state.
/// When V8 posts a foreground task for an isolate, the callback looks up
/// the state here, sets the notification flag, and wakes the event loop.
/// Isolates that received a notification before their state was registered
/// are tracked in `pending_wakes` so `register_isolate_waker` can wake
/// them immediately.
struct IsolateWakerRegistry {
  entries: HashMap<usize, IsolateWakeEntry>,
  pending_wakes: std::collections::HashSet<usize>,
}

static ISOLATE_WAKERS: std::sync::LazyLock<Mutex<IsolateWakerRegistry>> =
  std::sync::LazyLock::new(|| {
    Mutex::new(IsolateWakerRegistry {
      entries: HashMap::new(),
      pending_wakes: std::collections::HashSet::new(),
    })
  });

/// Register a waker and notification flag for an isolate so foreground
/// task notifications wake the correct event loop. If a notification
/// arrived before registration, the waker is triggered immediately.
pub fn register_isolate_waker(
  isolate_ptr: usize,
  waker: std::sync::Arc<AtomicWaker>,
) {
  let mut reg = ISOLATE_WAKERS.lock().unwrap();
  if reg.pending_wakes.remove(&isolate_ptr) {
    waker.wake();
  }
  reg.entries.insert(isolate_ptr, IsolateWakeEntry { waker });
}

/// Unregister an isolate's wake state (called on isolate drop).
pub fn unregister_isolate_waker(isolate_ptr: usize) {
  let mut reg = ISOLATE_WAKERS.lock().unwrap();
  reg.entries.remove(&isolate_ptr);
  reg.pending_wakes.remove(&isolate_ptr);
}

/// Wake the event loop for a given isolate. Sets the notification flag
/// and wakes the AtomicWaker. If the isolate's state is not yet
/// registered, marks it as pending so registration notifies it.
fn wake_isolate(key: usize) {
  let mut reg = ISOLATE_WAKERS.lock().unwrap();
  if let Some(entry) = reg.entries.get(&key) {
    entry.waker.wake();
  } else {
    reg.pending_wakes.insert(key);
  }
}

/// Entry in the delayed-task timer queue.
struct TimerEntry {
  deadline: Instant,
  isolate_key: usize,
}

impl PartialEq for TimerEntry {
  fn eq(&self, other: &Self) -> bool {
    self.deadline == other.deadline
  }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for TimerEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    // Reverse so BinaryHeap (max-heap) yields the earliest deadline first.
    other.deadline.cmp(&self.deadline)
  }
}

/// Single shared timer thread that processes all delayed V8 foreground
/// task wake-ups, avoiding one OS thread per delayed task.
static DELAYED_TASK_SENDER: std::sync::LazyLock<
  Mutex<std::sync::mpsc::Sender<TimerEntry>>,
> = std::sync::LazyLock::new(|| {
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::Builder::new()
    .name("deno-v8-timer".into())
    .spawn(move || delayed_task_thread(rx))
    .unwrap();
  Mutex::new(tx)
});

fn delayed_task_thread(rx: std::sync::mpsc::Receiver<TimerEntry>) {
  let mut heap: BinaryHeap<TimerEntry> = BinaryHeap::new();
  loop {
    // Block until either a new entry arrives or the next timer fires.
    if heap.is_empty() {
      match rx.recv() {
        Ok(entry) => heap.push(entry),
        Err(_) => break,
      }
    } else {
      let timeout = heap
        .peek()
        .unwrap()
        .deadline
        .saturating_duration_since(Instant::now());
      match rx.recv_timeout(timeout) {
        Ok(entry) => heap.push(entry),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
      }
    }

    // Drain any additional entries that arrived.
    while let Ok(entry) = rx.try_recv() {
      heap.push(entry);
    }

    // Fire all expired timers.
    let now = Instant::now();
    while let Some(entry) = heap.peek() {
      if entry.deadline <= now {
        let entry = heap.pop().unwrap();
        wake_isolate(entry.isolate_key);
      } else {
        break;
      }
    }
  }
}

/// Custom V8 platform implementation that wakes isolate event loops
/// when foreground tasks are posted from any thread (including V8
/// background compilation threads).
struct DenoPlatformImpl;

impl DenoPlatformImpl {
  fn wake_immediate(&self, isolate_ptr: *mut c_void) {
    wake_isolate(isolate_ptr as usize);
  }

  fn wake_delayed(&self, isolate_ptr: *mut c_void, delay_in_seconds: f64) {
    let entry = TimerEntry {
      deadline: Instant::now() + Duration::from_secs_f64(delay_in_seconds),
      isolate_key: isolate_ptr as usize,
    };
    let _ = DELAYED_TASK_SENDER.lock().unwrap().send(entry);
  }
}

impl v8::PlatformImpl for DenoPlatformImpl {
  fn post_task(&self, isolate_ptr: *mut c_void) {
    self.wake_immediate(isolate_ptr);
  }

  fn post_non_nestable_task(&self, isolate_ptr: *mut c_void) {
    self.wake_immediate(isolate_ptr);
  }

  fn post_delayed_task(&self, isolate_ptr: *mut c_void, delay_in_seconds: f64) {
    self.wake_delayed(isolate_ptr, delay_in_seconds);
  }

  fn post_non_nestable_delayed_task(
    &self,
    isolate_ptr: *mut c_void,
    delay_in_seconds: f64,
  ) {
    self.wake_delayed(isolate_ptr, delay_in_seconds);
  }

  fn post_idle_task(&self, isolate_ptr: *mut c_void) {
    self.wake_immediate(isolate_ptr);
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
    // Use a custom platform that notifies isolate event loops when V8
    // background threads post foreground tasks.
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
