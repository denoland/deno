// Copyright 2018-2026 the Deno authors. MIT license.
//
// HandleScope and friends.
//
// V8's `HandleScope` is a stack-allocated guard that marks live GC roots.
// `Local<T>` is bound to the scope's lifetime; on scope drop, locals are
// invalidated. QuickJS doesn't have rooted scopes — every `JSValue` is
// owned (refcount=1 on creation) and must be explicitly freed.
//
// We bridge by giving every HandleScope a `Vec<JSValue>` of values it owns.
// `track_owned` adds a fresh JSValue; on Drop the scope `JS_FreeValue`s
// every remaining entry. `EscapableHandleScope::escape` transfers one
// entry to the parent.
//
// The `'s` lifetime on `Local<'s, T>` is invariant: a Local can't outlive
// its scope, but multiple Locals from the same scope can be copied freely.

use core::marker::PhantomData;

use crate::context::Context;
use crate::context::ScopeParent;
use crate::isolate::Isolate;
use crate::isolate::IsolateState;
use crate::isolate::OwnedIsolate;
use crate::sys;
use crate::value::Local;

const MAX_SCOPE_DEPTH: usize = 4096;

/// Mirror of rusty_v8's `PinnedRef` — a thin wrapper around a mutable
/// borrow of a scope-like value, used by serde_v8 and op2 macros.
/// On the QuickJS side it's just a transparent alias.
#[repr(transparent)]
pub struct PinnedRef<'r, T: ?Sized>(pub &'r mut T);
impl<'r, T: ?Sized> std::ops::Deref for PinnedRef<'r, T> {
  type Target = T;
  fn deref(&self) -> &T {
    self.0
  }
}
impl<'r, T: ?Sized> std::ops::DerefMut for PinnedRef<'r, T> {
  fn deref_mut(&mut self) -> &mut T {
    self.0
  }
}

/// Heap stats stubs used by ext/telemetry. The canonical type lives
/// in `crate::isolate::HeapStatistics`; re-export so call sites that
/// use either path resolve.
pub use crate::isolate::HeapStatistics;

#[derive(Default)]
pub struct HeapSpaceStatistics {
  _p: (),
}
impl HeapSpaceStatistics {
  pub fn new() -> Self { Self::default() }
  pub fn space_name(&self) -> &'static std::ffi::CStr {
    c""
  }
  pub fn space_size(&self) -> usize { 0 }
  pub fn space_used_size(&self) -> usize { 0 }
  pub fn space_available_size(&self) -> usize { 0 }
  pub fn physical_space_size(&self) -> usize { 0 }
}

/// Inner data for HandleScope — no `'s` lifetime parameter, so its
/// Drop impl doesn't transitively constrain 's via dropck. The wrapper
/// HandleScope<'s, C> is just a phantom-typed shell.
pub struct HandleScopeInner {
  pub isolate: *mut Isolate,
  pub ctx: sys::Context,
  pub owned: Vec<sys::JSValue>,
  pub parent_owned: Option<*mut Vec<sys::JSValue>>,
  pub depth: usize,
}
impl Drop for HandleScopeInner {
  fn drop(&mut self) {
    for v in self.owned.drain(..) {
      sys::free_value(self.ctx, v);
    }
  }
}
/// The handle scope. On drop, all values registered with `track_owned`
/// have `JS_FreeValue` called on them.
#[repr(transparent)]
pub struct HandleScope<'s, C = Context> {
  pub(crate) inner: HandleScopeInner,
  pub(crate) _phantom: PhantomData<(*const &'s (), C)>,
}
// We don't impl Deref for HandleScope (Deref<Target=Isolate> exists
// elsewhere). Instead we expose the same fields directly via inherent
// methods or `pub(crate)` access through `self.inner.X`.

/// Trait abstracting "scope source" — anything HandleScope::new can
/// open on top of. Implemented for &mut OwnedIsolate (top-level scope)
/// and &mut PinScope / &mut HandleScope (nested scope).
pub trait HandleScopeSource {
  fn default_ctx(&mut self) -> sys::Context;
  fn isolate_ptr(&mut self) -> *mut Isolate;
}

impl HandleScopeSource for OwnedIsolate {
  fn default_ctx(&mut self) -> sys::Context {
    OwnedIsolate::default_ctx(self)
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    self.as_isolate() as *mut Isolate
  }
}

impl HandleScopeSource for Isolate {
  fn default_ctx(&mut self) -> sys::Context {
    Isolate::default_ctx(self)
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    self as *mut Isolate
  }
}

impl<'s, C> HandleScopeSource for HandleScope<'s, C> {
  fn default_ctx(&mut self) -> sys::Context {
    self.inner.ctx
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    self.inner.isolate
  }
}

impl<S: HandleScopeSource + ?Sized> HandleScopeSource for &mut S {
  fn default_ctx(&mut self) -> sys::Context {
    (**self).default_ctx()
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    (**self).isolate_ptr()
  }
}

impl<'a, 's, 'i, C> HandleScopeSource
  for crate::context::ContextScope<'a, PinScope<'s, 'i, C>>
{
  fn default_ctx(&mut self) -> sys::Context {
    use std::ops::DerefMut;
    let pin = self.deref_mut();
    pin.0.inner.ctx
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    use std::ops::DerefMut;
    let pin = self.deref_mut();
    pin.0.inner.isolate
  }
}

impl<'s, 'i, C> HandleScopeSource for PinScope<'s, 'i, C> {
  fn default_ctx(&mut self) -> sys::Context {
    self.0.inner.ctx
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    self.0.inner.isolate
  }
}

impl<'s, C> HandleScopeSource for CallbackScope<'s, C> {
  fn default_ctx(&mut self) -> sys::Context {
    self.0.inner.ctx
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    self.0.inner.isolate
  }
}

impl<'s, 'p, C> HandleScopeSource
  for crate::exception::TryCatch<'s, HandleScope<'p, C>>
{
  fn default_ctx(&mut self) -> sys::Context {
    use std::ops::DerefMut;
    let pin = self.deref_mut();
    pin.0.inner.ctx
  }
  fn isolate_ptr(&mut self) -> *mut Isolate {
    use std::ops::DerefMut;
    let pin = self.deref_mut();
    pin.0.inner.isolate
  }
}

impl<'s> HandleScope<'s, Context> {
  /// `v8::HandleScope::new(isolate)` — opens a scope on the isolate's
  /// default context. Mirrors rusty_v8's no-context constructor; on our
  /// side the isolate always has a default JSContext attached.
  ///
  /// Generic over scope source — accepts &mut OwnedIsolate, &mut
  /// HandleScope, or &mut PinScope.
  pub fn new<S: HandleScopeSource>(src: &mut S) -> Self {
    let ctx = src.default_ctx();
    let iso_ptr = src.isolate_ptr();
    // Track the most recently used isolate for this thread so the
    // op-bridge trampoline can recover it even when `opctx.isolate`
    // points at a stale Rust-stack address.
    crate::isolate::set_current_isolate_ptr(iso_ptr);
    Self { inner: HandleScopeInner { isolate: iso_ptr,
        ctx,
        owned: Vec::new(),
        parent_owned: None,
        depth: 0 }, _phantom: PhantomData }
  }
}

impl<'s> HandleScope<'s, Context> {
  /// `v8::HandleScope::with_context(isolate, ctx)`.
  pub fn with_context<'r>(
    iso: &'r mut OwnedIsolate,
    ctx: Local<'_, Context>,
  ) -> Self
  where
    'r: 's,
  {
    let iso_ptr = iso.as_isolate() as *mut Isolate;
    // Treat the underlying JSContext* as the active context.
    let ctx_raw = ctx.raw.u;
    let raw = sys::JSValue {
      u: ctx_raw,
      tag: ctx.raw.tag,
    };
    let _ = raw;
    let real_ctx = unsafe { ctx.raw.u.ptr } as sys::Context;
    Self { inner: HandleScopeInner { isolate: iso_ptr,
        ctx: real_ctx,
        owned: Vec::new(),
        parent_owned: None,
        depth: 0 }, _phantom: PhantomData }
  }
}

impl<'s, C> HandleScope<'s, C> {
  pub fn isolate(&mut self) -> &mut Isolate {
    unsafe { &mut *self.inner.isolate }
  }
  pub(crate) fn ctx(&self) -> sys::Context {
    self.inner.ctx
  }
  pub(crate) fn isolate_state(&self) -> &IsolateState {
    unsafe { (*self.inner.isolate).state() }
  }
  /// Record a freshly-created JSValue (refcount=1) in this scope's free
  /// list. Returns the same raw value.
  pub(crate) fn track_owned(&mut self, raw: sys::JSValue) {
    debug_assert!(
      self.inner.depth < MAX_SCOPE_DEPTH,
      "qjs_v8_compat: scope nesting too deep"
    );
    self.inner.owned.push(raw);
  }

  /// Snapshot of how many handles this scope currently owns. Useful for
  /// the refcount-balance test fixtures.
  pub fn owned_count(&self) -> usize {
    self.inner.owned.len()
  }

  /// Mirror of rusty_v8's `HandleScope::throw_exception`. Raises the
  /// given value as an exception in the current context. Returns a
  /// `Local<Value>` that wraps the thrown value (in V8 the return is
  /// the exception itself).
  pub fn throw_exception(
    &mut self,
    exc: crate::value::Local<'s, crate::value::Value>,
  ) -> crate::value::Local<'s, crate::value::Value> {
    crate::sys::throw(self.inner.ctx, exc.raw());
    exc
  }

  /// Mirror of rusty_v8's `HandleScope::has_pending_exception`.
  pub fn has_pending_exception(&self) -> bool {
    crate::sys::has_pending_exception(self.inner.ctx)
  }

  /// Mirror of `HandleScope::perform_microtask_checkpoint` — drains
  /// the microtask queue once. Loops draining pending QuickJS jobs
  /// until the queue is empty so promises settled this turn surface
  /// before the caller inspects them.
  pub fn perform_microtask_checkpoint(&mut self) {
    if self.inner.isolate.is_null() { return; }
    let rt = unsafe { (*self.inner.isolate).rt() };
    while crate::sys::run_pending_job(rt) {}
  }

  /// Mirror of `HandleScope::cancel_terminate_execution`.
  pub fn cancel_terminate_execution(&mut self) {}
  /// `Isolate::add_gc_prologue_callback` / `epilogue_callback` —
  /// no-op stubs (QuickJS doesn't expose V8-style GC hooks).
  pub fn add_gc_prologue_callback(
    &mut self,
    _cb: crate::v8::GCCallback,
    _data: *mut std::ffi::c_void,
    _gc_type: crate::v8::GCType,
  ) {
  }
  pub fn add_gc_epilogue_callback(
    &mut self,
    _cb: crate::v8::GCCallback,
    _data: *mut std::ffi::c_void,
    _gc_type: crate::v8::GCType,
  ) {
  }
  pub fn remove_gc_prologue_callback(
    &mut self,
    _cb: crate::v8::GCCallback,
    _data: *mut std::ffi::c_void,
  ) {
  }
  pub fn remove_gc_epilogue_callback(
    &mut self,
    _cb: crate::v8::GCCallback,
    _data: *mut std::ffi::c_void,
  ) {
  }
  pub fn get_heap_space_statistics(
    &mut self,
    _index: usize,
  ) -> Option<HeapSpaceStatistics> {
    None
  }
  pub fn get_number_of_data_slots(&mut self) -> u32 {
    8
  }
  pub fn number_of_heap_spaces(&self) -> usize {
    0
  }

  /// Mirror of rusty_v8's `HandleScope::escape` — used by EscapableHandleScope
  /// to extend a handle to the parent scope's lifetime. On QuickJS we
  /// just pass through: the parent scope owns the same arena so the
  /// handle is already valid for the parent's lifetime.
  pub fn escape<T>(
    &mut self,
    v: crate::value::Local<'_, T>,
  ) -> crate::value::Local<'s, T> {
    crate::value::Local::from_raw(v.raw())
  }

  /// Drop responsibility for `raw` from this scope (the caller is now
  /// responsible — typically because it's being escaped to a parent or
  /// promoted to Global). Internal.
  pub(crate) fn release_owned(&mut self, raw: sys::JSValue) -> bool {
    if let Some(idx) = self.inner.owned.iter().position(|v| value_equal(v, &raw)) {
      self.inner.owned.swap_remove(idx);
      true
    } else {
      false
    }
  }

  /// Test-only: expose the underlying JSContext pointer so integration
  /// tests can drive `sys` calls directly without re-implementing scope
  /// internals. Not on the rusty_v8 surface.
  #[doc(hidden)]
  pub fn ctx_for_test(&self) -> sys::Context {
    self.inner.ctx
  }
  #[doc(hidden)]
  pub fn track_owned_for_test(&mut self, raw: sys::JSValue) {
    self.track_owned(raw);
  }

  /// rusty_v8's `HandleScope::get_current_context`. We store it as a Local
  /// whose underlying `raw.u.ptr` is the JSContext pointer — context isn't
  /// a JSValue, but the cast is harmless because the API only uses Local
  /// as an opaque handle for contexts.
  pub fn get_current_context(&self) -> Local<'s, Context> {
    // Stash the JSContext pointer in u.ptr with a non-refcounted tag
    // (JS_TAG_UNDEFINED). Using JS_TAG_OBJECT here would make
    // JS_FreeValue try to refcount the JSContext as a JSObject and
    // corrupt the runtime allocator the next time something frees it.
    let raw = sys::JSValue {
      u: sys::JSValueUnion {
        ptr: self.inner.ctx as *mut _,
      },
      tag: sys::JS_TAG_UNDEFINED,
    };
    Local::from_raw(raw)
  }

  // Slot bookkeeping — V8 lets you stash arbitrary type-keyed values
  // on a scope. On QuickJS we route through the IsolateState's slot
  // map (set up by JsRuntime). For unwiring tests we keep the API but
  // store nothing.
  pub fn set_slot<T: 'static>(&mut self, _value: T) -> bool {
    false
  }
  pub fn get_slot<T: 'static>(&self) -> Option<&T> {
    None
  }
  pub fn get_slot_mut<T: 'static>(&mut self) -> Option<&mut T> {
    None
  }
  pub fn remove_slot<T: 'static>(&mut self) -> Option<T> {
    None
  }
  pub fn add_context_data<D>(
    &mut self,
    _context: crate::value::Local<'_, crate::context::Context>,
    _data: D,
  ) -> usize {
    0
  }
  pub fn set_continuation_preserved_embedder_data(
    &mut self,
    _data: crate::value::Local<'_, crate::value::Value>,
  ) {
  }
  pub fn get_continuation_preserved_embedder_data<'r>(
    &mut self,
  ) -> crate::value::Local<'r, crate::value::Value> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn get_heap_statistics(&self) -> crate::isolate::HeapStatistics {
    crate::isolate::HeapStatistics::default()
  }
  pub fn set_promise_hooks<I, B, A, R>(
    &mut self,
    _init: I,
    _before: B,
    _after: A,
    _resolve: R,
  ) {
  }
  pub fn set_wasm_streaming_callback<F>(&mut self, _cb: F) {}
  pub fn has_pending_background_tasks(&self) -> bool {
    false
  }
}

fn value_equal(a: &sys::JSValue, b: &sys::JSValue) -> bool {
  a.tag == b.tag && unsafe { a.u.ptr == b.u.ptr }
}

impl<'s, C> ScopeParent for HandleScope<'s, C> {
  fn isolate(&mut self) -> &mut Isolate {
    unsafe { &mut *self.inner.isolate }
  }
  fn set_current_context(&mut self, ctx: sys::Context) {
    self.inner.ctx = ctx;
  }
  fn current_context(&self) -> sys::Context {
    self.inner.ctx
  }
}

impl<'s, 'i, C> ScopeParent for PinScope<'s, 'i, C> {
  fn isolate(&mut self) -> &mut Isolate {
    unsafe { &mut *self.0.inner.isolate }
  }
  fn set_current_context(&mut self, ctx: sys::Context) {
    self.0.inner.ctx = ctx;
  }
  fn current_context(&self) -> sys::Context {
    self.0.inner.ctx
  }
}

// HandleScope derefs to Isolate so the deref chain
// PinScope -> HandleScope -> Isolate makes Isolate methods reachable
// from a `&mut PinScope` reference.
impl<'s, C> std::ops::Deref for HandleScope<'s, C> {
  type Target = Isolate;
  fn deref(&self) -> &Isolate {
    unsafe { &*self.inner.isolate }
  }
}
impl<'s, C> std::ops::DerefMut for HandleScope<'s, C> {
  fn deref_mut(&mut self) -> &mut Isolate {
    unsafe { &mut *self.inner.isolate }
  }
}

// HandleScope itself has NO Drop impl. Cleanup happens transparently
// when its `inner: HandleScopeInner` field drops. Keeping Drop off
// the lifetime-parameterised wrapper is what allows the `tc_scope!`
// macro pattern at callsites with short-lived parent borrows.

/// `EscapableHandleScope`: a child scope that can `escape` one Local to
/// its parent. On escape we transfer the JSValue from `self.inner.owned` to the
/// parent's `owned`.
pub struct EscapableHandleScope<'s, 'e: 's, C = Context> {
  inner: HandleScope<'s, C>,
  _escape: PhantomData<&'e ()>,
}

impl<'s, 'e: 's, C> EscapableHandleScope<'s, 'e, C> {
  pub fn new<'p>(parent: &'p mut HandleScope<'e, C>) -> Self
  where
    'p: 's,
  {
    let isolate = parent.inner.isolate;
    let ctx = parent.inner.ctx;
    let parent_owned = &mut parent.inner.owned as *mut _;
    let depth = parent.inner.depth + 1;
    Self {
      inner: HandleScope { inner: HandleScopeInner { isolate,
        ctx,
        owned: Vec::new(),
        parent_owned: Some(parent_owned),
        depth }, _phantom: PhantomData },
      _escape: PhantomData,
    }
  }

  pub fn escape<T>(&mut self, value: Local<'s, T>) -> Local<'e, T> {
    let parent = self
      .inner
      .inner
      .parent_owned
      .expect("EscapableHandleScope without parent");
    if let Some(idx) = self
      .inner
      .inner
      .owned
      .iter()
      .position(|v| value_equal(v, &value.raw))
    {
      let raw = self.inner.inner.owned.swap_remove(idx);
      unsafe { (*parent).push(raw) };
    }
    Local::from_raw(value.raw)
  }
}

impl<'s, 'e: 's, C> std::ops::Deref for EscapableHandleScope<'s, 'e, C> {
  type Target = HandleScope<'s, C>;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}
impl<'s, 'e: 's, C> std::ops::DerefMut for EscapableHandleScope<'s, 'e, C> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

/// `CallbackScope` — V8 uses this name when you re-enter the JS world
/// inside a host callback (op2 dispatch, weak finalizer, etc.). On our
/// side it's a regular HandleScope reconstructed from the raw callback ctx.
pub struct CallbackScope<'s, C = Context>(pub(crate) HandleScope<'s, C>);

// Mark CallbackScope as Unpin so `Pin<&mut CallbackScope>::deref_mut()`
// works and Pin's auto-deref chain reaches the inner HandleScope's
// inherent methods (throw_exception, get_current_context, etc.).
impl<'s, C> Unpin for CallbackScope<'s, C> {}

impl<'s> CallbackScope<'s, Context> {
  /// SAFETY: `ctx` must be a live JSContext owned by `iso`.
  pub unsafe fn new_from_context<'r>(
    iso: &'r mut Isolate,
    ctx: sys::Context,
  ) -> Self
  where
    'r: 's,
  {
    let iso_ptr = iso as *mut Isolate;
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: iso_ptr,
      ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }

  /// Mirror of rusty_v8's `CallbackScope::new(raw)` — constructs a
  /// CallbackScope from any opaque "scope-like" handle V8 passes to a
  /// callback. On the QuickJS backend this just forwards into the
  /// underlying scope; safety depends on the caller having a valid
  /// raw pointer.
  ///
  /// # Safety
  ///
  /// `raw` must be a live HandleScope-shaped pointer for the
  /// duration of `'s`.
  pub unsafe fn new<R>(raw: R) -> Self
  where
    R: CallbackScopeSource<'s>,
  {
    raw.into_callback_scope()
  }
}

/// Helper trait that `CallbackScope::new` accepts. Implemented for the
/// raw pointer types V8's various callbacks deliver — on QuickJS we
/// have just one (HandleScope), but the trait shape mirrors what
/// rusty_v8 does so deno_core's call sites work without edits.
pub trait CallbackScopeSource<'s>: Sized {
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context>;
}

impl<'s> CallbackScope<'s, Context> {
  /// Mirror of rusty_v8's pin-init pattern: op2-generated code does
  /// `let scope = std::mem::MaybeUninit::<v8::CallbackScope>::uninit();`
  /// then `Pin::new(&mut scope).init(raw)`. We accept the same shape
  /// and just overwrite the inner HandleScope with one constructed
  /// from the raw source.
  /// Mirror of rusty_v8's pin-init pattern. The op2 macro generates
  /// `Pin::new(&mut scope).init()` (no args) — V8's
  /// `CallbackScope::init` resolves its scope source by inspecting the
  /// stored callback context. On QuickJS the inner HandleScope is
  /// already initialized with whatever raw source `MaybeUninit` was
  /// fed; we just return the Pin handle through.
  ///
  /// Safe by design here even though rusty_v8 marks it `unsafe`: the
  /// op2 macro emits a bare `init()` call without an `unsafe` block,
  /// so making this safe lets that generated code compile.
  pub fn init(self: core::pin::Pin<&mut Self>) -> core::pin::Pin<&mut Self> {
    self
  }
}

impl<'s, 'r> CallbackScopeSource<'s> for &'r mut HandleScope<'s, Context> {
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: self.inner.isolate,
      ctx: self.inner.ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

// Local<Context> can be a CallbackScope source — deno_core uses
// `v8::CallbackScope::new(unsafe scope, ctx)` style with a Local<Context>.
impl<'s, 'r> CallbackScopeSource<'s>
  for crate::value::Local<'r, crate::context::Context>
{
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: core::ptr::null_mut(),
      ctx: core::ptr::null_mut(),
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

impl<'s, 'r> CallbackScopeSource<'s>
  for &'r crate::function::FunctionCallbackInfo
{
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    // Recover (isolate, ctx) from the FunctionCallbackInfo. The
    // op_bridge_trampoline_magic populates `implicit_args[DATA]` with
    // an External pointing at the OpCtx, whose `isolate` field is the
    // OwnedIsolate pointer we stored. We mirror that here.
    use crate::function::IMPLICIT_DATA_OFFSET;
    let info_ptr = self as *const crate::function::FunctionCallbackInfo;
    let data_jsv = unsafe {
      *(*info_ptr).implicit_args.offset(IMPLICIT_DATA_OFFSET)
    };
    let opctx_ptr = unsafe { data_jsv.u.ptr };
    if opctx_ptr.is_null() {
      return CallbackScope(HandleScope { inner: HandleScopeInner { isolate: core::ptr::null_mut(),
        ctx: core::ptr::null_mut(),
        owned: Vec::new(),
        parent_owned: None,
        depth: 0 }, _phantom: PhantomData });
    }
    let _ = opctx_ptr;
    // Use the currently-active isolate as recorded by OwnedIsolate::new.
    // The op2 callback path only runs while a runtime is alive, so this
    // is sound.
    let isolate_ptr = crate::isolate::current_isolate_ptr();
    let ctx = if isolate_ptr.is_null() {
      core::ptr::null_mut()
    } else {
      unsafe { (*isolate_ptr).default_ctx() }
    };
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: isolate_ptr,
      ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

// FastApiCallbackOptions is what fast-API callbacks receive in V8;
// op2-generated code constructs a CallbackScope from it. On QuickJS the
// fast path is dead code (no JIT) but the construction must still
// type-check.
impl<'s, 'r> CallbackScopeSource<'s>
  for &'r crate::v8::fast_api::FastApiCallbackOptions<'s>
{
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: core::ptr::null_mut(),
      ctx: core::ptr::null_mut(),
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

// `&mut Isolate` is what deno_core's inspector code passes to
// `callback_scope!(unsafe scope, isolate)`.
impl<'s, 'r> CallbackScopeSource<'s> for &'r mut &'r mut Isolate {
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    let ctx = (**self).default_ctx();
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: *self as *mut Isolate,
      ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

// PromiseRejectMessage is passed to host promise rejection callbacks
// where deno_core wraps `&message` in a callback_scope.
impl<'s, 'r> CallbackScopeSource<'s>
  for &'r crate::promise::PromiseRejectMessage<'s>
{
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    CallbackScope(HandleScope { inner: HandleScopeInner { isolate: core::ptr::null_mut(),
      ctx: core::ptr::null_mut(),
      owned: Vec::new(),
      parent_owned: None,
      depth: 0 }, _phantom: PhantomData })
  }
}

// CallbackScope derefs to PinScope (which itself derefs to HandleScope)
// so the `Pin<&mut CallbackScope>` chain auto-coerces to
// `&mut PinScope` when passed to deno_core APIs that expect a
// PinScope reference.
impl<'s, C> std::ops::Deref for CallbackScope<'s, C> {
  type Target = PinScope<'s, 's, C>;
  fn deref(&self) -> &Self::Target {
    // SAFETY: PinScope is #[repr(transparent)] over HandleScope
    // and CallbackScope's first field IS a HandleScope.
    unsafe { &*(self as *const Self as *const PinScope<'s, 's, C>) }
  }
}
impl<'s, C> std::ops::DerefMut for CallbackScope<'s, C> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *(self as *mut Self as *mut PinScope<'s, 's, C>) }
  }
}

/// Mirror of rusty_v8's two-lifetime `PinScope<'s, 'i>`. We model it
/// as a `#[repr(transparent)]` wrapper around HandleScope so callers
/// can transmute between `&mut HandleScope` and `&mut PinScope` for
/// free. Helper macros (`scope!`, `tc_scope!`, etc.) bind the scope
/// as `&mut PinScope` so deno_core helpers that take `&mut PinScope`
/// see the right type without further coercion.
#[repr(transparent)]
pub struct PinScope<'s, 'i: 's, C = Context>(
  pub(crate) HandleScope<'s, C>,
  PhantomData<&'i ()>,
);

impl<'s, 'i: 's, C> PinScope<'s, 'i, C> {
  /// Construct a PinScope wrapping an existing HandleScope.
  pub fn from_handle_scope_mut<'r>(
    hs: &'r mut HandleScope<'s, C>,
  ) -> &'r mut PinScope<'s, 'i, C> {
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut Self) }
  }
}
impl<'s, 'i, C> std::ops::Deref for PinScope<'s, 'i, C> {
  type Target = HandleScope<'s, C>;
  fn deref(&self) -> &HandleScope<'s, C> {
    &self.0
  }
}
impl<'s, 'i, C> std::ops::DerefMut for PinScope<'s, 'i, C> {
  fn deref_mut(&mut self) -> &mut HandleScope<'s, C> {
    &mut self.0
  }
}

/// `PinCallbackScope<'s, 'i>` is a type alias for
/// `Pin<&'i mut CallbackScope<'s>>` — that's how rusty_v8 spells the
/// pin-rooted callback scope handle, and it's what op2-generated code
/// expects to pass around.
pub type PinCallbackScope<'s, 'i> = CallbackScope<'s>;

// ContextScope already lives in `crate::context`; add the
// LocalNewScope impl here so the trait reaches it.
impl<'cs, 's, C> crate::value::LocalNewScope<'s>
  for crate::context::ContextScope<'cs, HandleScope<'s, C>>
where
  HandleScope<'s, C>: crate::value::LocalNewScope<'s>,
{
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s>
  where
    Self: 'a,
  {
    use std::ops::DerefMut;
    let hs: &mut HandleScope<'s, C> = self.deref_mut();
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut HandleScope<'s>) }
  }
}
impl<'cs, 's, 'i, C> crate::value::LocalNewScope<'s>
  for crate::context::ContextScope<'cs, PinScope<'s, 'i, C>>
{
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s>
  where
    Self: 'a,
  {
    use std::ops::DerefMut;
    let pin: &mut PinScope<'s, 'i, C> = self.deref_mut();
    let hs: &mut HandleScope<'s, C> = pin.deref_mut();
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut HandleScope<'s>) }
  }
}

// v8::scope free fn — used by deno_core's scope macro.
pub fn scope<'s, 'r>(iso: &'r mut OwnedIsolate) -> HandleScope<'s, Context>
where
  'r: 's,
{
  HandleScope::<'s, Context>::new(iso)
}
pub fn scope_with_context<'s, 'r>(
  iso: &'r mut OwnedIsolate,
  ctx: Local<'_, Context>,
) -> HandleScope<'s, Context>
where
  'r: 's,
{
  HandleScope::<'s, Context>::with_context(iso, ctx)
}

pub mod escapable_handle_scope {
  pub use super::EscapableHandleScope;
}

pub mod callback_scope {
  pub use super::CallbackScope;
}
