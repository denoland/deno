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

/// The handle scope. On drop, all values registered with `track_owned`
/// have `JS_FreeValue` called on them.
pub struct HandleScope<'s, C = Context> {
  pub(crate) isolate: *mut Isolate,
  pub(crate) ctx: sys::Context,
  pub(crate) owned: Vec<sys::JSValue>,
  /// For `EscapableHandleScope::escape`: a pointer up to the parent's
  /// `owned` vec. `None` for top-level scopes.
  pub(crate) parent_owned: Option<*mut Vec<sys::JSValue>>,
  pub(crate) depth: usize,
  _scope: PhantomData<&'s mut ()>,
  _ctx: PhantomData<C>,
}

impl<'s> HandleScope<'s, Context> {
  /// `v8::HandleScope::new(isolate)` — opens a scope on the isolate's
  /// default context. Mirrors rusty_v8's no-context constructor; on our
  /// side the isolate always has a default JSContext attached.
  pub fn new<'r>(iso: &'r mut OwnedIsolate) -> Self
  where
    'r: 's,
  {
    let ctx = iso.default_ctx();
    let iso_ptr = iso.as_isolate() as *mut Isolate;
    Self {
      isolate: iso_ptr,
      ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    }
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
    Self {
      isolate: iso_ptr,
      ctx: real_ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    }
  }
}

impl<'s, C> HandleScope<'s, C> {
  pub fn isolate(&mut self) -> &mut Isolate {
    unsafe { &mut *self.isolate }
  }
  pub(crate) fn ctx(&self) -> sys::Context {
    self.ctx
  }
  pub(crate) fn isolate_state(&self) -> &IsolateState {
    unsafe { (*self.isolate).state() }
  }
  /// Record a freshly-created JSValue (refcount=1) in this scope's free
  /// list. Returns the same raw value.
  pub(crate) fn track_owned(&mut self, raw: sys::JSValue) {
    debug_assert!(
      self.depth < MAX_SCOPE_DEPTH,
      "qjs_v8_compat: scope nesting too deep"
    );
    self.owned.push(raw);
  }

  /// Snapshot of how many handles this scope currently owns. Useful for
  /// the refcount-balance test fixtures.
  pub fn owned_count(&self) -> usize {
    self.owned.len()
  }

  /// Mirror of rusty_v8's `HandleScope::throw_exception`. Raises the
  /// given value as an exception in the current context. Returns a
  /// `Local<Value>` that wraps the thrown value (in V8 the return is
  /// the exception itself).
  pub fn throw_exception(
    &mut self,
    exc: crate::value::Local<'s, crate::value::Value>,
  ) -> crate::value::Local<'s, crate::value::Value> {
    crate::sys::throw(self.ctx, exc.raw());
    exc
  }

  /// Mirror of rusty_v8's `HandleScope::has_pending_exception`.
  pub fn has_pending_exception(&self) -> bool {
    crate::sys::has_pending_exception(self.ctx)
  }

  /// Mirror of `HandleScope::perform_microtask_checkpoint` — drains
  /// the microtask queue once. Stubbed; the QuickJS event loop bridge
  /// drives microtasks separately via `JS_ExecutePendingJob`.
  pub fn perform_microtask_checkpoint(&mut self) {}

  /// Mirror of `HandleScope::cancel_terminate_execution`.
  pub fn cancel_terminate_execution(&mut self) {}

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
    if let Some(idx) = self.owned.iter().position(|v| value_equal(v, &raw)) {
      self.owned.swap_remove(idx);
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
    self.ctx
  }
  #[doc(hidden)]
  pub fn track_owned_for_test(&mut self, raw: sys::JSValue) {
    self.track_owned(raw);
  }

  /// rusty_v8's `HandleScope::get_current_context`. We store it as a Local
  /// whose underlying `raw.u.ptr` is the JSContext pointer — context isn't
  /// a JSValue, but the cast is harmless because the API only uses Local
  /// as an opaque handle for contexts.
  pub fn get_current_context(&mut self) -> Local<'s, Context> {
    let raw = sys::JSValue {
      u: sys::JSValueUnion {
        ptr: self.ctx as *mut _,
      },
      tag: sys::JS_TAG_OBJECT,
    };
    Local::from_raw(raw)
  }
}

fn value_equal(a: &sys::JSValue, b: &sys::JSValue) -> bool {
  a.tag == b.tag && unsafe { a.u.ptr == b.u.ptr }
}

impl<'s, C> ScopeParent for HandleScope<'s, C> {
  fn isolate(&mut self) -> &mut Isolate {
    unsafe { &mut *self.isolate }
  }
  fn set_current_context(&mut self, ctx: sys::Context) {
    self.ctx = ctx;
  }
  fn current_context(&self) -> sys::Context {
    self.ctx
  }
}

impl<'s, C> Drop for HandleScope<'s, C> {
  fn drop(&mut self) {
    for v in self.owned.drain(..) {
      sys::free_value(self.ctx, v);
    }
  }
}

/// `EscapableHandleScope`: a child scope that can `escape` one Local to
/// its parent. On escape we transfer the JSValue from `self.owned` to the
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
    let isolate = parent.isolate;
    let ctx = parent.ctx;
    let parent_owned = &mut parent.owned as *mut _;
    let depth = parent.depth + 1;
    Self {
      inner: HandleScope {
        isolate,
        ctx,
        owned: Vec::new(),
        parent_owned: Some(parent_owned),
        depth,
        _scope: PhantomData,
        _ctx: PhantomData,
      },
      _escape: PhantomData,
    }
  }

  pub fn escape<T>(&mut self, value: Local<'s, T>) -> Local<'e, T> {
    let parent = self
      .inner
      .parent_owned
      .expect("EscapableHandleScope without parent");
    if let Some(idx) = self
      .inner
      .owned
      .iter()
      .position(|v| value_equal(v, &value.raw))
    {
      let raw = self.inner.owned.swap_remove(idx);
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
    CallbackScope(HandleScope {
      isolate: iso_ptr,
      ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    })
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
  pub unsafe fn init(
    self: core::pin::Pin<&mut Self>,
  ) -> core::pin::Pin<&mut Self> {
    self
  }
}

impl<'s, 'r> CallbackScopeSource<'s> for &'r mut HandleScope<'s, Context> {
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    CallbackScope(HandleScope {
      isolate: self.isolate,
      ctx: self.ctx,
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    })
  }
}

impl<'s, 'r> CallbackScopeSource<'s>
  for &'r crate::function::FunctionCallbackInfo
{
  unsafe fn into_callback_scope(self) -> CallbackScope<'s, Context> {
    // The op2 callback path doesn't actually have a HandleScope it can
    // borrow from here; this is best-effort. The CallbackScope it
    // returns has zero ctx — any use of it under QuickJS is unsupported.
    CallbackScope(HandleScope {
      isolate: core::ptr::null_mut(),
      ctx: core::ptr::null_mut(),
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    })
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
    CallbackScope(HandleScope {
      isolate: core::ptr::null_mut(),
      ctx: core::ptr::null_mut(),
      owned: Vec::new(),
      parent_owned: None,
      depth: 0,
      _scope: PhantomData,
      _ctx: PhantomData,
    })
  }
}

impl<'s, C> std::ops::Deref for CallbackScope<'s, C> {
  type Target = HandleScope<'s, C>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl<'s, C> std::ops::DerefMut for CallbackScope<'s, C> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// Mirror of rusty_v8's two-lifetime `PinScope<'s, 'i>`. Both lifetimes
/// collapse to one on QuickJS — we don't enforce the pinning hygiene
/// rusty_v8 uses, but the call-site signatures must accept both args.
/// Rust doesn't support defaulting lifetime params on type aliases, so
/// we just take them all and ignore the ones we don't use.
pub type PinScope<'s, 'i, C = Context> = HandleScope<'s, C>;
pub type PinCallbackScope<'s, 'i, C = Context> = CallbackScope<'s, C>;

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
