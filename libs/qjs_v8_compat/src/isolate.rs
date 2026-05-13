// Copyright 2018-2026 the Deno authors. MIT license.
//
// Isolate / OwnedIsolate.
//
// In V8 an `Isolate` owns the JS heap; in QuickJS the analogous object is
// `JSRuntime`. A `Context` (V8 `Context` == QuickJS `JSContext`) lives inside
// an Isolate/Runtime.
//
// We mirror rusty_v8's split between `Isolate` (a borrowed view) and
// `OwnedIsolate` (the owning RAII handle).

use core::ffi::c_void;
use core::ptr::NonNull;
use std::cell::RefCell;
use std::sync::Arc;

use crate::sys;

/// Backing data we attach to every Isolate. Stored in the runtime's opaque
/// pointer so `&mut Isolate` and `Local<T>` can both reach it without
/// threading state through every call.
pub(crate) struct IsolateState {
  pub microtasks_policy: MicrotasksPolicy,
  /// Promise-rejection hook (compat with v8::PromiseRejectCallback).
  pub promise_reject_cb: Option<PromiseRejectCallback>,
  /// Slot pointers (v8::Isolate::set_data / get_data).
  pub data_slots: [*mut c_void; 8],
}

impl IsolateState {
  fn new() -> Self {
    Self {
      microtasks_policy: MicrotasksPolicy::Auto,
      promise_reject_cb: None,
      data_slots: [core::ptr::null_mut(); 8],
    }
  }
}

#[derive(Default)]
pub struct CreateParams {
  pub heap_limits: Option<(usize, usize)>,
}
impl CreateParams {
  pub fn heap_limits(mut self, initial: usize, max: usize) -> Self {
    self.heap_limits = Some((initial, max));
    self
  }
  pub fn array_buffer_allocator_shared<T>(self, _alloc: T) -> Self {
    self
  }
  pub fn embedder_wrapper_type_info_offsets(self, _a: i32, _b: i32) -> Self {
    self
  }
  pub fn allow_atomics_wait(self, _allow: bool) -> Self {
    self
  }
}

pub enum MicrotasksPolicy {
  Auto,
  Explicit,
  Scoped,
}

/// `OwnedIsolate` is the RAII wrapper around a runtime + default context. On
/// drop, frees the context and runtime in that order. The compat layer
/// stores backing state in a leaked `Box<IsolateState>` referenced via the
/// runtime opaque pointer.
pub struct OwnedIsolate {
  rt: sys::Runtime,
  default_ctx: sys::Context,
  // Held alive for the lifetime of the runtime; freed in `Drop`.
  state: Option<Box<IsolateState>>,
}

unsafe impl Send for OwnedIsolate {}

impl OwnedIsolate {
  pub fn new(_params: CreateParams) -> Self {
    let rt = sys::new_runtime();
    let ctx = sys::new_context(rt);
    let mut state = Box::new(IsolateState::new());
    let state_ptr: *mut IsolateState = state.as_mut();
    sys::set_runtime_opaque(rt, state_ptr as *mut c_void);
    Self {
      rt,
      default_ctx: ctx,
      state: Some(state),
    }
  }

  pub fn thread_safe_handle(&self) -> IsolateHandle {
    IsolateHandle { rt: self.rt }
  }

  pub fn set_promise_reject_callback(&mut self, cb: PromiseRejectCallback) {
    self.state.as_mut().unwrap().promise_reject_cb = Some(cb);
  }

  pub fn set_microtasks_policy(&mut self, policy: MicrotasksPolicy) {
    self.state.as_mut().unwrap().microtasks_policy = policy;
  }

  /// V8 has `Isolate::perform_microtask_checkpoint`. On QuickJS this means
  /// draining the pending job queue.
  pub fn perform_microtask_checkpoint(&mut self) {
    while sys::run_pending_job(self.rt) {}
  }

  /// Underlying runtime pointer. Used by scope-creation helpers; not part
  /// of the public v8 surface.
  pub(crate) fn rt(&self) -> sys::Runtime {
    self.rt
  }

  pub(crate) fn default_ctx(&self) -> sys::Context {
    self.default_ctx
  }

  /// View this isolate as `&mut Isolate`. Mirrors V8's `*mut Isolate` deref.
  pub fn as_isolate(&mut self) -> &mut Isolate {
    // SAFETY: Isolate is a transparent newtype around OwnedIsolate's
    // internal pointer; we hand out a reborrow tied to self's lifetime.
    unsafe { &mut *(self as *mut OwnedIsolate as *mut Isolate) }
  }
}

impl Drop for OwnedIsolate {
  fn drop(&mut self) {
    sys::free_context(self.default_ctx);
    sys::free_runtime(self.rt);
    // state Box is dropped here.
    drop(self.state.take());
  }
}

/// Borrowed isolate. Functionally identical to `OwnedIsolate` in API surface
/// (rusty_v8 expresses many operations as `&mut Isolate`), but holds no
/// drop responsibility.
#[repr(transparent)]
pub struct Isolate(OwnedIsolate);

impl Isolate {
  pub fn thread_safe_handle(&self) -> IsolateHandle {
    self.0.thread_safe_handle()
  }
  pub fn perform_microtask_checkpoint(&mut self) {
    self.0.perform_microtask_checkpoint()
  }
  pub(crate) fn rt(&self) -> sys::Runtime {
    self.0.rt
  }
  pub(crate) fn default_ctx(&self) -> sys::Context {
    self.0.default_ctx
  }
  pub fn cancel_terminate_execution(&mut self) {}
  pub fn terminate_execution(&mut self) -> bool {
    false
  }
  pub fn is_execution_terminating(&mut self) -> bool {
    false
  }
  pub fn enter(&mut self) {}
  pub fn exit(&mut self) {}
  /// Throw `value` into the active context. Mirrors V8's
  /// `Isolate::ThrowException`. Returns `undefined` so the caller can
  /// pass it straight through as the function return value.
  ///
  /// QuickJS's `JS_Throw` transfers one refcount to the runtime's
  /// pending-exception slot; we `JS_DupValue` first so the caller's
  /// `Local` stays valid (its scope still owns the original refcount).
  pub fn throw_exception<'s>(
    &mut self,
    value: crate::value::Local<'s, crate::value::Value>,
  ) -> crate::value::Local<'s, crate::value::Value> {
    let ctx = self.0.default_ctx;
    let raw = sys::dup_value(ctx, value.raw());
    let _ = sys::throw(ctx, raw);
    crate::value::Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_data(&self, slot: u32) -> *mut c_void {
    let s = self.state();
    s.data_slots
      .get(slot as usize)
      .copied()
      .unwrap_or(core::ptr::null_mut())
  }
  pub fn set_data(&mut self, slot: u32, data: *mut c_void) {
    let s = unsafe { self.state_mut() };
    if let Some(p) = s.data_slots.get_mut(slot as usize) {
      *p = data;
    }
  }
  pub(crate) fn state(&self) -> &IsolateState {
    let p = sys::get_runtime_opaque(self.0.rt) as *const IsolateState;
    assert!(!p.is_null(), "OwnedIsolate state was nulled out");
    unsafe { &*p }
  }
  pub(crate) unsafe fn state_mut(&mut self) -> &mut IsolateState {
    let p = sys::get_runtime_opaque(self.0.rt) as *mut IsolateState;
    assert!(!p.is_null(), "OwnedIsolate state was nulled out");
    unsafe { &mut *p }
  }
}

/// Send/Sync handle to an isolate that can be used to request termination
/// from another thread. QuickJS has `JS_RequestInterrupt`; we wire it
/// later. For now it's just a tagged pointer.
#[derive(Clone)]
pub struct IsolateHandle {
  rt: sys::Runtime,
}
unsafe impl Send for IsolateHandle {}
unsafe impl Sync for IsolateHandle {}

impl IsolateHandle {
  pub fn terminate_execution(&self) -> bool {
    // TODO: wire JS_RequestInterrupt
    false
  }
}

/// Marker for the unsafe raw isolate pointer that some op2 paths take. We
/// stash the raw `JSContext*` so that the equivalent code can resume on the
/// QuickJS side.
#[derive(Copy, Clone)]
pub struct UnsafeRawIsolatePtr(pub *mut c_void);
unsafe impl Send for UnsafeRawIsolatePtr {}
unsafe impl Sync for UnsafeRawIsolatePtr {}

/// V8's promise-rejection callback signature is a `extern "C" fn` taking a
/// `PromiseRejectMessage`. For the compat layer we use a thin Rust trait
/// object so the QuickJS-side dispatcher can route without going through C.
pub type PromiseRejectCallback =
  fn(message: crate::promise::PromiseRejectMessage<'_>);

// `Platform` is a Rust-side placeholder. Real V8 uses libplatform; QuickJS
// has no equivalent and runs all work on the embedder's thread.
pub type Platform = ();

thread_local! {
  /// rusty_v8 has implicit isolate context. We replicate the slot so code
  /// that calls helpers without an explicit `&mut Isolate` (e.g. error
  /// constructors) can still locate the runtime.
  pub(crate) static CURRENT_ISOLATE: RefCell<Option<Arc<core::cell::Cell<*mut Isolate>>>>
    = const { RefCell::new(None) };
}

pub(crate) fn enter_isolate(iso: &mut Isolate) -> IsolateGuard {
  let cell = Arc::new(core::cell::Cell::new(iso as *mut Isolate));
  CURRENT_ISOLATE.with(|c| *c.borrow_mut() = Some(cell.clone()));
  IsolateGuard { _cell: cell }
}

pub(crate) struct IsolateGuard {
  _cell: Arc<core::cell::Cell<*mut Isolate>>,
}
impl Drop for IsolateGuard {
  fn drop(&mut self) {
    CURRENT_ISOLATE.with(|c| *c.borrow_mut() = None);
  }
}

/// Public NonNull pointer mirror used by deno_core's `JsRuntime` struct.
pub type IsolatePtr = NonNull<Isolate>;
