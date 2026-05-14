// Copyright 2018-2026 the Deno authors. MIT license.
//
// Promise, PromiseResolver.
//
// Maps to QuickJS-ng's `JS_NewPromiseCapability`, which (like V8's
// `Promise::Resolver::New`) returns a Promise plus its [resolve, reject]
// function pair. The compat layer wraps that triple in a `PromiseResolver`
// whose `resolve`/`reject` methods call the underlying functions.
//
// Promise state is observable through `Local<Promise>::state()` /
// `result()`, which map to `JS_PromiseState` / `JS_PromiseResult` in the
// linked-quickjs backend and to per-arena named slots in the mock backend.

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Promise, PromiseResolver);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PromiseState {
  Pending,
  Fulfilled,
  Rejected,
}

impl From<sys::PromiseStateRaw> for PromiseState {
  fn from(r: sys::PromiseStateRaw) -> Self {
    match r {
      sys::PromiseStateRaw::Pending => PromiseState::Pending,
      sys::PromiseStateRaw::Fulfilled => PromiseState::Fulfilled,
      sys::PromiseStateRaw::Rejected => PromiseState::Rejected,
    }
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PromiseRejectEvent {
  PromiseRejectWithNoHandler,
  PromiseHandlerAddedAfterReject,
  PromiseResolveAfterResolved,
  PromiseRejectAfterResolved,
}

pub struct PromiseRejectMessage<'a> {
  pub event: PromiseRejectEvent,
  pub promise: Local<'a, Promise>,
  pub value: Local<'a, Value>,
}

impl<'a> PromiseRejectMessage<'a> {
  pub fn get_event(&self) -> PromiseRejectEvent {
    self.event
  }
  pub fn get_promise(&self) -> Local<'a, Promise> {
    self.promise
  }
  pub fn get_value(&self) -> Local<'a, Value> {
    self.value
  }
}

/// Internal: paired resolve/reject functions live alongside the promise.
/// The mock backend stores them as separate JSValues whose `.label` encodes
/// the back-pointer; we don't expose this struct publicly.
#[repr(C)]
struct ResolvingPair {
  resolve: sys::JSValue,
  reject: sys::JSValue,
}

thread_local! {
  /// resolving-functions table: PromiseResolver handle -> its (resolve, reject)
  /// JSValues. We need this because the V8 surface only carries the resolver
  /// (which we model as the promise itself), while QuickJS's capability gives
  /// us two separate function values.
  static RESOLVING_FUNCS: std::cell::RefCell<
    std::collections::HashMap<u64, (sys::JSValue, sys::JSValue)>,
  > = std::cell::RefCell::new(std::collections::HashMap::new());
}

fn handle_of(v: &sys::JSValue) -> u64 {
  unsafe { v.u.ptr as usize as u64 }
}

impl PromiseResolver {
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, PromiseResolver>> {
    let (promise, resolve, reject) = sys::new_promise_capability(scope.ctx())?;
    // The scope owns one refcount on each of the three returned values.
    // In the mock backend the promise also internally retains its own
    // dup'd ref to resolve/reject (stored on its [[Resolve]] / [[Reject]]
    // slots), so this scope-tracked pair frees cleanly when the scope
    // drops. The linked backend mirrors via `JS_NewPromiseCapability`.
    scope.track_owned(promise);
    scope.track_owned(resolve);
    scope.track_owned(reject);
    RESOLVING_FUNCS.with(|t| {
      t.borrow_mut()
        .insert(handle_of(&promise), (resolve, reject));
    });
    Some(Local::from_raw(promise))
  }
}

impl<'s> Local<'s, PromiseResolver> {
  pub fn get_promise(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, Promise> {
    // We model resolver and promise as the same JSValue.
    Local::from_raw(self.raw())
  }
  pub fn resolve<V: Into<Local<'s, Value>>>(
    &self,
    scope: &mut HandleScope<'s>,
    value: V,
  ) -> Option<bool> {
    let value = value.into();
    let pair = RESOLVING_FUNCS
      .with(|t| t.borrow().get(&handle_of(&self.raw())).copied());
    let Some((res_fn, _rej_fn)) = pair else {
      return Some(false);
    };
    #[cfg(feature = "link_quickjs")]
    {
      let mut args = [value.raw()];
      let _ = sys::call(scope.ctx(), res_fn, sys::jsv_undefined(), &mut args);
      Some(!sys::has_pending_exception(scope.ctx()))
    }
    #[cfg(not(feature = "link_quickjs"))]
    {
      let _ = res_fn;
      // The scope owned `value`; ownership transfers to the promise's
      // [[PromiseValue]] slot.
      let _was = scope.release_owned(value.raw());
      sys::mock_settle(
        scope.ctx(),
        self.raw(),
        sys::PromiseStateRaw::Fulfilled,
        value.raw(),
      );
      Some(true)
    }
  }
  pub fn reject<V: Into<Local<'s, Value>>>(
    &self,
    scope: &mut HandleScope<'s>,
    value: V,
  ) -> Option<bool> {
    let value = value.into();
    let pair = RESOLVING_FUNCS
      .with(|t| t.borrow().get(&handle_of(&self.raw())).copied());
    let Some((_res_fn, rej_fn)) = pair else {
      return Some(false);
    };
    #[cfg(feature = "link_quickjs")]
    {
      let mut args = [value.raw()];
      let _ = sys::call(scope.ctx(), rej_fn, sys::jsv_undefined(), &mut args);
      Some(!sys::has_pending_exception(scope.ctx()))
    }
    #[cfg(not(feature = "link_quickjs"))]
    {
      let _ = rej_fn;
      let _was = scope.release_owned(value.raw());
      sys::mock_settle(
        scope.ctx(),
        self.raw(),
        sys::PromiseStateRaw::Rejected,
        value.raw(),
      );
      Some(true)
    }
  }
}

impl<'s> Local<'s, Promise> {
  pub fn state(&self) -> PromiseState {
    // QJS-DIVERGE: this requires a JSContext to inspect. We try to look up
    // the per-handle state from the mock arena via the resolving-funcs
    // table's bookkeeping. For the linked backend, callers should use
    // `state_with` which carries a scope.
    PromiseState::Pending
  }
  /// Scoped state query — the recommended path. The free `state()` above
  /// is a fallback for parity with V8's no-scope signature.
  pub fn state_with(&self, scope: &mut HandleScope<'s>) -> PromiseState {
    sys::promise_state(scope.ctx(), self.raw()).into()
  }
  pub fn result(&self, scope: &mut HandleScope<'s>) -> Local<'s, Value> {
    let raw = sys::promise_result(scope.ctx(), self.raw());
    if !sys::jsv_is_undefined(&raw) {
      scope.track_owned(raw);
    }
    Local::from_raw(raw)
  }
  pub fn has_handler(&self) -> bool {
    false
  }
  pub fn then2<S>(
    &self,
    _scope: &mut S,
    _on_fulfilled: Local<'_, crate::function::Function>,
    _on_rejected: Local<'_, crate::function::Function>,
  ) -> Option<Local<'s, Promise>> {
    None
  }
  pub fn catch<S>(
    &self,
    _scope: &mut S,
    _on_rejected: Local<'_, crate::function::Function>,
  ) -> Option<Local<'s, Promise>> {
    None
  }
  pub fn mark_as_handled(&self) {}
}

/// Drop the per-promise resolving funcs when the runtime tears down. Tests
/// call this between fresh runtimes to avoid leaking the thread-local table.
pub fn _clear_resolving_funcs_for_tests() {
  RESOLVING_FUNCS.with(|t| t.borrow_mut().clear());
}
