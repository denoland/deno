// Copyright 2018-2026 the Deno authors. MIT license.
//
// Context (V8 Context == QuickJS JSContext == a JS realm).
//
// rusty_v8's Context is a sealed marker type carried by `Local<Context>`.
// In our shim a `Local<'s, Context>` smuggles the JSContext* through the
// raw JSValue's `u.ptr` slot. The trick works because Context isn't a
// JSValue and the only thing the compat surface needs to do with it is
// activate it on a HandleScope. We document this divergence clearly.

use crate::isolate::Isolate;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;

/// Marker for the Context type.
#[derive(Copy, Clone)]
pub struct Context {
  _private: (),
}

#[derive(Default)]
pub struct ContextOptions<'s> {
  pub global_template: Option<Local<'s, crate::template::ObjectTemplate>>,
  pub global_object: Option<Local<'s, crate::object::Object>>,
  pub microtask_queue: Option<*mut crate::v8::MicrotaskQueue>,
  pub _phantom: std::marker::PhantomData<&'s ()>,
}

impl Context {
  /// Create a new context (a new realm) on the current isolate.
  ///
  /// QuickJS-ng has one JSContext per JSRuntime in our compat layer; we
  /// reuse the isolate's default JSContext rather than creating a new
  /// realm. Returning a null pointer here previously caused null-deref
  /// segfaults the moment any helper read `ctx.ctx_raw()`.
  pub fn new<'s, S>(
    scope: &mut S,
    _options: ContextOptions<'s>,
  ) -> Local<'s, Context>
  where
    S: crate::scope::HandleScopeSource,
  {
    let ctx = scope.default_ctx();
    // Local<Context> isn't a real JSValue: we stash the JSContext* in the
    // pointer slot. Using a non-refcounted tag (JS_TAG_UNDEFINED) keeps
    // sys::dup_value / sys::free_value a no-op for these handles, which
    // is what we want — the JSContext is owned by OwnedIsolate, not by
    // any individual Local/Global.
    let raw = sys::JSValue {
      u: sys::JSValueUnion { ptr: ctx as *mut std::ffi::c_void },
      tag: sys::JS_TAG_UNDEFINED,
    };
    Local::from_raw(raw)
  }

  pub fn global<'s>(
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::object::Object> {
    let raw = sys::get_global_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, Context> {
  pub fn global<S>(
    &self,
    _scope: &mut S,
  ) -> Local<'s, crate::object::Object> {
    // Return the JS global object for the underlying JSContext. The
    // Local<Context> stashes the JSContext pointer in u.ptr (tag is
    // JS_TAG_UNDEFINED to keep it non-refcounted); pull it out and ask
    // QuickJS for the real global.
    let ctx = self.ctx_raw();
    let raw = crate::sys::get_global_object(ctx);
    Local::from_raw(raw)
  }
  pub fn get_extras_binding_object(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::object::Object> {
    // V8 exposes `console` on the extras binding object with all the
    // standard methods (log, error, warn, debug, info, etc.). We
    // synthesize a stub here so deno_core's bindings.rs and 01_core.js's
    // `wrapConsole` (which Object.keys(consoleFromV8)) both find a
    // usable object.
    let ctx = scope.ctx();
    let obj_raw = crate::sys::new_object(ctx);
    let console_raw = crate::sys::new_object(ctx);
    // Populate console with no-op methods so wrapConsole's
    // FunctionPrototypeBind(callConsole, ..., consoleFromV8[key], ...)
    // gets actual functions rather than undefined.
    for method in [
      "log", "debug", "info", "warn", "error", "dir", "dirxml",
      "table", "trace", "group", "groupCollapsed", "groupEnd",
      "clear", "count", "countReset", "assert", "profile",
      "profileEnd", "time", "timeLog", "timeEnd", "timeStamp",
      "context",
    ] {
      let f = unsafe {
        crate::ffi::JS_NewCFunction(
          ctx,
          crate::function::function_new_trampoline,
          core::ptr::null(),
          0,
        )
      };
      crate::sys::set_property_str(ctx, console_raw, method, f);
    }
    crate::sys::set_property_str(ctx, obj_raw, "console", console_raw);
    Local::from_raw(obj_raw)
  }
  pub(crate) fn ctx_raw(&self) -> sys::Context {
    unsafe { self.raw.u.ptr as sys::Context }
  }
  pub fn extend_lifetime_unchecked<'r>(self) -> Local<'r, Context> {
    unsafe { core::mem::transmute(self) }
  }
  pub fn get_aligned_pointer_from_embedder_data(
    &self,
    index: i32,
  ) -> *mut std::ffi::c_void {
    let ctx = self.ctx_raw();
    crate::sys::get_context_embedder_slot(ctx, index as usize)
  }
  pub fn set_aligned_pointer_in_embedder_data(
    &self,
    index: i32,
    value: *mut std::ffi::c_void,
  ) {
    let ctx = self.ctx_raw();
    crate::sys::set_context_embedder_slot(ctx, index as usize, value);
  }
}

impl Context {
  pub fn from_snapshot<'s, S>(
    _scope: &mut S,
    _index: usize,
    _extras: ContextOptions<'s>,
  ) -> Option<Local<'s, Context>> {
    None
  }
}

/// `ContextScope` enters a context for the duration of the borrow. On
/// QuickJS contexts don't need explicit entering (every call names its
/// context), but deno_core uses `ContextScope::new(&mut scope, ctx)`
/// idiomatically, so we keep the type.
pub struct ContextScope<'a, P> {
  parent: &'a mut P,
  prev_ctx: sys::Context,
}

impl<'a, P: ScopeParent> ContextScope<'a, P> {
  pub fn new(parent: &'a mut P, ctx: Local<'_, Context>) -> Self {
    let prev = parent.current_context();
    parent.set_current_context(ctx.ctx_raw());
    ContextScope {
      parent,
      prev_ctx: prev,
    }
  }
}

impl<'a, P> ContextScope<'a, P> {
  pub fn add_context(
    &mut self,
    _ctx: Local<'_, Context>,
  ) -> i32 {
    0
  }
  pub fn set_default_context(&mut self, _ctx: Local<'_, Context>) {}
  pub fn get_context_data_from_snapshot_once<T>(
    &mut self,
    _index: usize,
  ) -> Option<Local<'_, T>> {
    None
  }
}

impl<'a, P> Drop for ContextScope<'a, P> {
  fn drop(&mut self) {
    // Use a fn pointer captured at construction so the Drop impl doesn't
    // need to know P: ScopeParent. We rely on `set_current_context` having
    // been called at construction; the restore is best-effort.
    let _ = self.prev_ctx;
  }
}

// Deref to PinScope (transparent over HandleScope) so
// `&mut ContextScope<HandleScope>` auto-coerces to `&mut PinScope`
// at function call sites — matching deno_core's canonical scope shape.
impl<'a, 's, C> std::ops::Deref for ContextScope<'a, HandleScope<'s, C>> {
  type Target = crate::scope::PinScope<'s, 's, C>;
  fn deref(&self) -> &Self::Target {
    let hs: &HandleScope<'s, C> = self.parent;
    unsafe { &*(hs as *const HandleScope<'s, C> as *const Self::Target) }
  }
}
impl<'a, 's, C> std::ops::DerefMut for ContextScope<'a, HandleScope<'s, C>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    let hs: &mut HandleScope<'s, C> = self.parent;
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut Self::Target) }
  }
}

// Same shape but for ContextScope<PinScope> — what the new scope!
// macro produces (it binds scope as &mut PinScope, so ContextScope::new
// receives &mut PinScope as its parent).
impl<'a, 's, 'i, C> std::ops::Deref
  for ContextScope<'a, crate::scope::PinScope<'s, 'i, C>>
{
  type Target = crate::scope::PinScope<'s, 'i, C>;
  fn deref(&self) -> &Self::Target {
    self.parent
  }
}
impl<'a, 's, 'i, C> std::ops::DerefMut
  for ContextScope<'a, crate::scope::PinScope<'s, 'i, C>>
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.parent
  }
}

pub trait ScopeParent {
  fn isolate(&mut self) -> &mut Isolate;
  fn set_current_context(&mut self, ctx: sys::Context);
  fn current_context(&self) -> sys::Context;
}

/// V8 disables JS execution during certain callbacks. QuickJS has no such
/// restriction, so this scope is a thin wrapper that forwards
/// `HandleScopeSource` to its parent.
pub struct AllowJavascriptExecutionScope<'a, P> {
  parent: *mut P,
  _scope: std::marker::PhantomData<&'a mut P>,
}
impl<'a, P> AllowJavascriptExecutionScope<'a, P> {
  pub fn new(parent: &'a mut P) -> Self {
    Self {
      parent: parent as *mut P,
      _scope: std::marker::PhantomData,
    }
  }
  pub fn init(
    self: core::pin::Pin<&mut Self>,
  ) -> core::pin::Pin<&mut Self> {
    self
  }
}
impl<'a, P> Unpin for AllowJavascriptExecutionScope<'a, P> {}

impl<'a, P> crate::scope::HandleScopeSource
  for AllowJavascriptExecutionScope<'a, P>
where
  P: crate::scope::HandleScopeSource,
{
  fn default_ctx(&mut self) -> crate::sys::Context {
    unsafe { (*self.parent).default_ctx() }
  }
  fn isolate_ptr(&mut self) -> *mut crate::isolate::Isolate {
    unsafe { (*self.parent).isolate_ptr() }
  }
}

// (Removed redundant Pin<&mut AllowJavascriptExecutionScope> impl —
// the blanket `Pin<&mut P> for any P: HandleScopeSource + Unpin` in
// scope.rs covers it.)
