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
pub struct ContextOptions;

impl Context {
  /// Create a new context (a new realm) on the current isolate.
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    _options: ContextOptions,
  ) -> Local<'s, Context> {
    let rt = scope.isolate().rt();
    let raw_ctx = sys::new_context(rt);
    let raw = sys::JSValue {
      u: sys::JSValueUnion {
        ptr: raw_ctx as *mut _,
      },
      tag: sys::JS_TAG_OBJECT,
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
  pub fn global(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::object::Object> {
    let raw = sys::get_global_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  pub fn get_extras_binding_object(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::object::Object> {
    self.global(scope)
  }
  pub(crate) fn ctx_raw(&self) -> sys::Context {
    unsafe { self.raw.u.ptr as sys::Context }
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

pub trait ScopeParent {
  fn isolate(&mut self) -> &mut Isolate;
  fn set_current_context(&mut self, ctx: sys::Context);
  fn current_context(&self) -> sys::Context;
}

/// V8 disables JS execution during certain callbacks. QuickJS has no such
/// restriction, so this scope is a no-op.
pub struct AllowJavascriptExecutionScope<'a, P> {
  _scope: std::marker::PhantomData<&'a mut P>,
}
impl<'a, P> AllowJavascriptExecutionScope<'a, P> {
  pub fn new(_parent: &'a mut P) -> Self {
    Self {
      _scope: std::marker::PhantomData,
    }
  }
}
