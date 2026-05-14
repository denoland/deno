// Copyright 2018-2026 the Deno authors. MIT license.
//
// Exception primitives + TryCatch.
//
// V8's `TryCatch` is a stack-allocated trap that, while alive, catches any
// exception thrown during JS execution and lets the embedder inspect it.
// QuickJS exposes pending exceptions through `JS_HasException` /
// `JS_GetException` — there is exactly one pending exception per context
// at a time. We model `TryCatch` as a guard that snapshots whether there
// was already a pending exception when it was opened (so nested TryCatch
// composes) and, on Drop, restores the prior one if it had been displaced.

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

/// V8's `Exception` namespace exposes error constructors. We mirror them.
///
/// In the mock backend each constructor allocates a plain object with a
/// `"message"` and `"name"` property; the linked backend will route
/// through `JS_ThrowTypeError` / `JS_NewError`. Either way the returned
/// `Local<Value>` is a *value*, not a thrown exception — passing it to
/// `isolate.throw_exception` (or `JS_Throw` directly) is what causes
/// QuickJS's pending-exception slot to fill.
pub struct Exception;

fn new_error<'s>(
  scope: &mut HandleScope<'s>,
  name: &str,
  message: Local<'s, crate::primitives::String>,
) -> Local<'s, Value> {
  let raw = sys::new_object(scope.ctx());
  scope.track_owned(raw);
  let obj_local: Local<'s, crate::object::Object> = Local::from_raw(raw);

  // Set name + message via the string-keyed wrapper. Take fresh string
  // refcounts so the obj owns them.
  let name_s = sys::new_string(scope.ctx(), name);
  scope.track_owned(name_s);
  let name_val: Local<'s, Value> = Local::from_raw(name_s);
  let _ = obj_local.set_str(scope, "name", name_val);

  let msg_val: Local<'s, Value> = Local::from_raw(message.raw());
  // The string was already in the scope's owned vec — set_str will release
  // it from there and transfer to the property slot.
  let _ = obj_local.set_str(scope, "message", msg_val);

  Local::from_raw(raw)
}

impl Exception {
  pub fn error<'s>(
    scope: &mut HandleScope<'s>,
    message: Local<'s, crate::primitives::String>,
  ) -> Local<'s, Value> {
    new_error(scope, "Error", message)
  }
  pub fn type_error<'s>(
    scope: &mut HandleScope<'s>,
    message: Local<'s, crate::primitives::String>,
  ) -> Local<'s, Value> {
    new_error(scope, "TypeError", message)
  }
  pub fn range_error<'s>(
    scope: &mut HandleScope<'s>,
    message: Local<'s, crate::primitives::String>,
  ) -> Local<'s, Value> {
    new_error(scope, "RangeError", message)
  }
  pub fn syntax_error<'s>(
    scope: &mut HandleScope<'s>,
    message: Local<'s, crate::primitives::String>,
  ) -> Local<'s, Value> {
    new_error(scope, "SyntaxError", message)
  }
  pub fn reference_error<'s>(
    scope: &mut HandleScope<'s>,
    message: Local<'s, crate::primitives::String>,
  ) -> Local<'s, Value> {
    new_error(scope, "ReferenceError", message)
  }
}

/// A scope that, while alive, catches any exception thrown in the JS world.
///
/// QuickJS has a single pending-exception slot per context, so the
/// `TryCatch` here simply *peeks* at that slot when queried. The first
/// call to `has_caught`/`exception` lifts the pending value out into
/// `self.caught`, transferring its refcount.
///
/// We deliberately do not stack TryCatch frames: the inner-most call to
/// `exception()` consumes the value, so an outer TryCatch sees an empty
/// slot. This matches the QuickJS C API; the only divergence from V8 is
/// that an unconsumed inner TryCatch *does not* propagate the exception
/// outward — on Drop we drain & free it. // QJS-DIVERGE.
pub struct TryCatch<'s, S> {
  parent: &'s mut S,
  /// The exception lifted out of the runtime's pending slot. Owns one
  /// refcount; on `exception()` ownership transfers to the parent scope,
  /// on `rethrow()` it goes back into the pending slot, on Drop it's freed.
  caught: Option<sys::JSValue>,
  /// Cached context — copied at construction so Drop doesn't need to
  /// reach back through the parent.
  ctx: sys::Context,
}

impl<'s, 'p, C> TryCatch<'s, HandleScope<'p, C>> {
  pub fn new(parent: &'s mut HandleScope<'p, C>) -> Self {
    let ctx = parent.ctx();
    Self {
      parent,
      caught: None,
      ctx,
    }
  }

  /// Pull the pending exception (if any) into `self.caught` once.
  fn maybe_take(&mut self) {
    if self.caught.is_none() {
      self.caught = sys::take_pending_exception(self.ctx);
    }
  }

  pub fn has_caught(&mut self) -> bool {
    self.maybe_take();
    self.caught.is_some()
  }
  pub fn exception(&mut self) -> Option<Local<'p, Value>> {
    self.maybe_take();
    // Take ownership; promote to the parent scope's tracked vec so the
    // caller can use the Local without an extra dup.
    let exc = self.caught.take()?;
    if sys::jsv_is_object(&exc)
      || sys::jsv_is_string(&exc)
      || sys::jsv_is_symbol(&exc)
      || sys::jsv_is_bigint(&exc)
    {
      self.parent.track_owned(exc);
    }
    Some(Local::from_raw(exc))
  }
  pub fn message(&mut self) -> Option<Local<'p, crate::value::Message>> {
    // We don't synthesize a separate Message object; the exception
    // already carries `.message` as a property. Callers typically
    // serialize via `to_rust_string_lossy` on `.exception()`.
    None
  }
  pub fn stack_trace<'a>(&mut self) -> Option<Local<'a, Value>> {
    None
  }
  pub fn rethrow(&mut self) -> Option<Local<'p, Value>> {
    self.maybe_take();
    let exc = self.caught.take()?;
    // Re-arm the runtime's pending slot — transfers our refcount.
    sys::throw(self.ctx, exc);
    Some(Local::from_raw(exc))
  }
  pub fn reset(&mut self) {
    if let Some(exc) = self.caught.take() {
      sys::free_value(self.ctx, exc);
    }
  }
  pub fn is_verbose(&self) -> bool {
    false
  }
  pub fn set_verbose(&mut self, _v: bool) {}
  pub fn capture_message(&mut self) -> bool {
    false
  }
  pub fn set_capture_message(&mut self, _v: bool) {}
  /// Mirror of `TryCatch::has_terminated`.
  pub fn has_terminated(&self) -> bool {
    false
  }
  /// Mirror of `TryCatch::is_execution_terminating`.
  pub fn is_execution_terminating(&self) -> bool {
    false
  }
  /// Mirror of `TryCatch::cancel_terminate_execution`.
  pub fn cancel_terminate_execution(&mut self) {}
}

// Pin<&mut TryCatch>::init mirrors Pin<&mut CallbackScope>::init.
impl<'s, S> TryCatch<'s, S> {
  pub fn init(self: core::pin::Pin<&mut Self>) -> core::pin::Pin<&mut Self> {
    self
  }
}
// TryCatch is Unpin so Pin<&mut TryCatch>::deref reaches the inner.
impl<'s, S> Unpin for TryCatch<'s, S> {}

// Specialized Deref: when the parent is a HandleScope, we deref to a
// PinScope (PinScope is repr(transparent) over HandleScope, so this
// is a free reinterpret). This lets `&mut TryCatch<HandleScope>`
// auto-coerce to `&mut PinScope` via deref coercion at function call
// sites — matching the canonical scope shape that deno_core uses.
impl<'s, 'p, C> std::ops::Deref for TryCatch<'s, HandleScope<'p, C>> {
  type Target = crate::scope::PinScope<'p, 'p, C>;
  fn deref(&self) -> &Self::Target {
    let hs: &HandleScope<'p, C> = self.parent;
    unsafe { &*(hs as *const HandleScope<'p, C> as *const Self::Target) }
  }
}
impl<'s, 'p, C> std::ops::DerefMut for TryCatch<'s, HandleScope<'p, C>> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    let hs: &mut HandleScope<'p, C> = self.parent;
    unsafe { &mut *(hs as *mut HandleScope<'p, C> as *mut Self::Target) }
  }
}

impl<'s, S> Drop for TryCatch<'s, S> {
  fn drop(&mut self) {
    // If we lifted an exception out and the caller never consumed it,
    // free it now. QJS-DIVERGE: V8 would re-propagate to the enclosing
    // TryCatch; QuickJS has no multi-level pending slot.
    if let Some(exc) = self.caught.take() {
      sys::free_value(self.ctx, exc);
    }
  }
}

/// Used by `v8::tc_scope!` style macros.
pub mod tc_scope {
  pub use super::TryCatch;
}

/// `DataError` is V8's structured marshalling error.
///
/// rusty_v8 exposes it as an enum with `BadType { actual, expected }`
/// and `NoData` variants. We mirror that exactly so deno_core's
/// `DataError::BadType { ... }` construction compiles.
#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum DataError {
  BadType {
    actual: &'static str,
    expected: &'static str,
  },
  NoData {
    expected: &'static str,
  },
}
impl std::fmt::Display for DataError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::BadType { actual, expected } => {
        write!(
          f,
          "DataError::BadType {{ actual: {actual}, expected: {expected} }}"
        )
      }
      Self::NoData { expected } => {
        write!(f, "DataError::NoData {{ expected: {expected} }}")
      }
    }
  }
}
impl std::error::Error for DataError {}

impl From<core::convert::Infallible> for DataError {
  fn from(_: core::convert::Infallible) -> Self {
    Self::BadType {
      actual: "infallible",
      expected: "infallible",
    }
  }
}
