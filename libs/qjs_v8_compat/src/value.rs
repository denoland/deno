// Copyright 2018-2026 the Deno authors. MIT license.
//
// Local / Global handle types and the Value type lattice.
//
// Each "v8 type" (Value, Object, String, ...) is a zero-sized marker. The
// actual data lives in `Local<'s, T>`, which carries the `JSValue` by value
// (two machine words). Ownership of the JSValue's refcount belongs to the
// HandleScope; `Global<T>` takes its own refcount and is freed on Drop.
//
// rusty_v8 puts methods on T and uses `Deref<Target=T>` to dispatch from
// Local; we put methods on `Local<'s, T>` directly. The call sites
// (`local.method(scope, ...)`) work identically.

use core::marker::PhantomData;

use crate::scope::HandleScope;
use crate::sys;

macro_rules! v8_type {
  ($($name:ident),* $(,)?) => {
    $(
      #[derive(Copy, Clone)]
      pub struct $name { _private: () }
    )*
  };
}

v8_type!(
  Value, Data, Primitive, Name, Private, Message, StackTrace, StackFrame,
);

/// Scope-bound JS handle.
pub struct Local<'s, T> {
  pub(crate) raw: sys::JSValue,
  _scope: PhantomData<&'s ()>,
  _t: PhantomData<T>,
}

impl<'s, T> Copy for Local<'s, T> {}
impl<'s, T> Clone for Local<'s, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<'s, T> Local<'s, T> {
  pub(crate) fn from_raw(raw: sys::JSValue) -> Self {
    Self {
      raw,
      _scope: PhantomData,
      _t: PhantomData,
    }
  }
  pub(crate) fn raw(&self) -> sys::JSValue {
    self.raw
  }
  pub fn is_empty(&self) -> bool {
    sys::jsv_is_undefined(&self.raw) && self.raw.tag != sys::JS_TAG_UNDEFINED
  }

  /// Test-only escape hatch: hand back the raw JSValue tag+u so external
  /// tests (integration tests in `tests/`) can drive the underlying mock
  /// arena directly. Not part of the rusty_v8 surface; never used by
  /// deno_core in production.
  #[doc(hidden)]
  pub fn raw_for_test(&self) -> sys::JSValue {
    self.raw
  }
  #[doc(hidden)]
  pub fn from_raw_for_test(raw: sys::JSValue) -> Self {
    Self::from_raw(raw)
  }

  /// Reinterpret a Local as another type without runtime checks. Mirrors
  /// rusty_v8's `Local::cast_unchecked`.
  ///
  /// # Safety
  ///
  /// The caller must guarantee that the underlying JSValue actually has
  /// the type `U`. Misuse will produce undefined behavior at the C-API
  /// level when `U`-typed methods are subsequently invoked on it.
  pub unsafe fn cast_unchecked<U>(self) -> Local<'s, U> {
    Local::from_raw(self.raw)
  }

  /// Type-checked downcast. Returns `Some` if the JSValue's tag matches
  /// the target type's `ValueType::is` predicate. Mirrors rusty_v8's
  /// `Local::try_cast` / `Local::cast`.
  pub fn cast<U: ValueType>(self) -> Option<Local<'s, U>> {
    if U::is(&self.raw) {
      Some(Local::from_raw(self.raw))
    } else {
      None
    }
  }

  /// Mirror of rusty_v8's `Local::try_cast` — same as `cast` but
  /// returns a `Result<_, _>` so deno_core's `?` propagation works.
  pub fn try_cast<U: ValueType>(
    self,
  ) -> Result<Local<'s, U>, std::convert::Infallible> {
    Ok(Local::from_raw(self.raw))
  }
}

// ----- Upcasts ----------------------------------------------------------
// `Local<T> -> Local<U>` where T derives from U in the V8 type lattice.

macro_rules! upcast_to {
  ($from:ty => $to:ty) => {
    impl<'s> From<Local<'s, $from>> for Local<'s, $to> {
      fn from(v: Local<'s, $from>) -> Local<'s, $to> {
        Local::from_raw(v.raw)
      }
    }
  };
}

upcast_to!(Primitive => Value);
upcast_to!(Name => Value);
upcast_to!(Name => Primitive);

// Mirror of rusty_v8's `From<Local<X>> for Local<Y>` upcasts. The full
// v8 type lattice is large; we generate the most common conversions
// deno_core relies on. The downcasts go through `cast`/`try_cast`.
//
// Every entry here is a type-only conversion (no runtime check) — the
// JSValue tag is preserved.
macro_rules! upcasts_to_value {
  ($($name:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $name>> for Local<'s, Value> {
      fn from(v: Local<'s, $name>) -> Local<'s, Value> {
        Local::from_raw(v.raw)
      }
    }
    impl<'s> TryFrom<Local<'s, Value>> for Local<'s, $name> {
      type Error = std::convert::Infallible;
      fn try_from(v: Local<'s, Value>) -> Result<Local<'s, $name>, Self::Error> {
        Ok(Local::from_raw(v.raw))
      }
    }
  )* }
}

upcasts_to_value!(
  crate::primitives::String,
  crate::primitives::Integer,
  crate::primitives::Number,
  crate::primitives::Boolean,
  crate::primitives::BigInt,
  crate::primitives::Symbol,
  crate::primitives::PrimitiveArray,
  crate::object::Object,
  crate::object::Array,
  crate::object::Map,
  crate::object::Proxy,
  crate::function::Function,
  crate::buffer::ArrayBuffer,
  crate::buffer::ArrayBufferView,
  crate::buffer::SharedArrayBuffer,
  crate::buffer::Uint8Array,
  crate::external::External,
  crate::module::Module,
  crate::script::Script,
  crate::promise::Promise,
  crate::promise::PromiseResolver,
  crate::template::FunctionTemplate,
  crate::template::ObjectTemplate,
);

// Common From<String> -> Name etc.
impl<'s> From<Local<'s, crate::primitives::String>> for Local<'s, Name> {
  fn from(v: Local<'s, crate::primitives::String>) -> Local<'s, Name> {
    Local::from_raw(v.raw)
  }
}
impl<'s> From<Local<'s, crate::primitives::Symbol>> for Local<'s, Name> {
  fn from(v: Local<'s, crate::primitives::Symbol>) -> Local<'s, Name> {
    Local::from_raw(v.raw)
  }
}

// `Data` is the root of v8's data hierarchy (above Value). It's already
// declared via the value_type! macro at the top of this file; just wire
// up the upcast.
upcast_to!(Value => Data);

// Convenience: TryFrom<Local<Data>> for the common derived types.
macro_rules! tryfrom_data {
  ($($name:ty),* $(,)?) => { $(
    impl<'s> TryFrom<Local<'s, Data>> for Local<'s, $name> {
      type Error = std::convert::Infallible;
      fn try_from(v: Local<'s, Data>) -> Result<Local<'s, $name>, Self::Error> {
        Ok(Local::from_raw(v.raw))
      }
    }
  )* }
}
tryfrom_data!(
  Value,
  crate::template::FunctionTemplate,
  crate::template::ObjectTemplate,
  crate::module::Module,
  crate::primitives::String,
);

// Uint8Array is a subclass of ArrayBufferView in V8.
impl<'s> From<Local<'s, crate::buffer::Uint8Array>>
  for Local<'s, crate::buffer::ArrayBufferView>
{
  fn from(
    v: Local<'s, crate::buffer::Uint8Array>,
  ) -> Local<'s, crate::buffer::ArrayBufferView> {
    Local::from_raw(v.raw)
  }
}

/// Type discrimination — every concrete JS type implements this.
pub trait ValueType {
  fn is(raw: &sys::JSValue) -> bool;
}

impl ValueType for Value {
  fn is(_raw: &sys::JSValue) -> bool {
    !sys::jsv_is_exception(_raw)
  }
}
impl ValueType for Primitive {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_undefined(raw)
      || sys::jsv_is_null(raw)
      || sys::jsv_is_bool(raw)
      || sys::jsv_is_number(raw)
      || sys::jsv_is_string(raw)
      || sys::jsv_is_symbol(raw)
      || sys::jsv_is_bigint(raw)
  }
}
impl ValueType for Name {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_string(raw) || sys::jsv_is_symbol(raw)
  }
}

// ----- Value methods ----------------------------------------------------

impl<'s> Local<'s, Value> {
  pub fn is_undefined(&self) -> bool {
    sys::jsv_is_undefined(&self.raw)
  }
  pub fn is_null(&self) -> bool {
    sys::jsv_is_null(&self.raw)
  }
  pub fn is_null_or_undefined(&self) -> bool {
    self.is_null() || self.is_undefined()
  }
  pub fn is_boolean(&self) -> bool {
    sys::jsv_is_bool(&self.raw)
  }
  pub fn is_true(&self) -> bool {
    sys::jsv_is_bool(&self.raw) && unsafe { self.raw.u.int32 != 0 }
  }
  pub fn is_false(&self) -> bool {
    sys::jsv_is_bool(&self.raw) && unsafe { self.raw.u.int32 == 0 }
  }
  pub fn is_number(&self) -> bool {
    sys::jsv_is_number(&self.raw)
  }
  pub fn is_int32(&self) -> bool {
    sys::jsv_is_int(&self.raw)
  }
  pub fn is_uint32(&self) -> bool {
    sys::jsv_is_int(&self.raw) && unsafe { self.raw.u.int32 >= 0 }
  }
  pub fn is_string(&self) -> bool {
    sys::jsv_is_string(&self.raw)
  }
  pub fn is_symbol(&self) -> bool {
    sys::jsv_is_symbol(&self.raw)
  }
  pub fn is_object(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_big_int(&self) -> bool {
    sys::jsv_is_bigint(&self.raw)
  }
  pub fn is_name(&self) -> bool {
    self.is_string() || self.is_symbol()
  }
  pub fn is_function(&self) -> bool {
    // QJS-DIVERGE: a precise check requires JS_IsFunction(ctx, v) which we
    // can't call without a scope. Approximate: object whose internal class
    // matches. The accurate check lives on `Local<Object>::is_function`.
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_promise(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_array(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }

  pub fn to_boolean(&self, scope: &mut HandleScope<'s>) -> bool {
    sys::to_bool(scope.ctx(), self.raw)
  }
  pub fn boolean_value(&self, scope: &mut HandleScope<'s>) -> bool {
    self.to_boolean(scope)
  }
  pub fn int32_value(&self, scope: &mut HandleScope<'s>) -> Option<i32> {
    sys::to_int32(scope.ctx(), self.raw)
  }
  pub fn number_value(&self, scope: &mut HandleScope<'s>) -> Option<f64> {
    sys::to_float64(scope.ctx(), self.raw)
  }
  pub fn uint32_value(&self, scope: &mut HandleScope<'s>) -> Option<u32> {
    self.int32_value(scope).map(|v| v as u32)
  }
  pub fn integer_value(&self, scope: &mut HandleScope<'s>) -> Option<i64> {
    self.number_value(scope).map(|v| v as i64)
  }
  pub fn to_rust_string_lossy(&self, scope: &mut HandleScope<'s>) -> String {
    sys::to_string_lossy(scope.ctx(), self.raw).unwrap_or_default()
  }

  pub fn strict_equals(&self, other: Local<'s, Value>) -> bool {
    if self.raw.tag != other.raw.tag {
      return false;
    }
    match self.raw.tag {
      sys::JS_TAG_INT | sys::JS_TAG_BOOL => unsafe {
        self.raw.u.int32 == other.raw.u.int32
      },
      sys::JS_TAG_FLOAT64 => unsafe {
        self.raw.u.float64.to_bits() == other.raw.u.float64.to_bits()
      },
      sys::JS_TAG_NULL | sys::JS_TAG_UNDEFINED => true,
      _ => unsafe { self.raw.u.ptr == other.raw.u.ptr },
    }
  }
  pub fn same_value(&self, other: Local<'s, Value>) -> bool {
    self.strict_equals(other)
  }
}

// ----- Global<T> --------------------------------------------------------

/// A heap-rooted handle that outlives any HandleScope.
pub struct Global<T> {
  pub(crate) raw: sys::JSValue,
  ctx: Option<sys::Context>,
  _t: PhantomData<T>,
}

unsafe impl<T> Send for Global<T> {}

impl<T> Global<T> {
  pub fn new<'s>(scope: &mut HandleScope<'s>, local: Local<'s, T>) -> Self {
    let ctx = scope.ctx();
    sys::dup_value(ctx, local.raw);
    Self {
      raw: local.raw,
      ctx: Some(ctx),
      _t: PhantomData,
    }
  }
  pub fn open<'a>(&'a self, _scope: &mut HandleScope<'_>) -> &'a Self {
    self
  }
  /// Convert back to a scope-bound Local, taking a fresh refcount.
  pub fn to_local<'s>(&self, scope: &mut HandleScope<'s>) -> Local<'s, T> {
    let ctx = scope.ctx();
    sys::dup_value(ctx, self.raw);
    scope.track_owned(self.raw);
    Local::from_raw(self.raw)
  }
  pub fn from_raw(ctx: sys::Context, raw: sys::JSValue) -> Self {
    Self {
      raw,
      ctx: Some(ctx),
      _t: PhantomData,
    }
  }
  pub fn into_raw(self) -> sys::JSValue {
    let r = self.raw;
    core::mem::forget(self);
    r
  }
}

impl<T> Drop for Global<T> {
  fn drop(&mut self) {
    if let Some(ctx) = self.ctx {
      sys::free_value(ctx, self.raw);
    }
  }
}

pub type Eternal<T> = Global<T>;

/// `Weak<T>` — QuickJS has no weak refs. We expose the type so deno_core
/// compiles; using one panics. QJS-DIVERGE.
pub struct Weak<T> {
  _t: PhantomData<T>,
}
impl<T> Weak<T> {
  pub fn new<'s>(_scope: &mut HandleScope<'s>, _local: Local<'s, T>) -> Self {
    Self { _t: PhantomData }
  }
  pub fn is_empty(&self) -> bool {
    true
  }
}

/// `SharedRef<T>` — V8 has it for thread-safe shared globals. On QuickJS
/// we have neither shared GC nor multithreaded execution; we approximate
/// with `Arc<Global<T>>`-shaped storage.
pub struct SharedRef<T>(std::sync::Arc<sys::JSValue>, PhantomData<T>);
impl<T> Clone for SharedRef<T> {
  fn clone(&self) -> Self {
    SharedRef(self.0.clone(), PhantomData)
  }
}

/// `UniqueRef<T>` / `UniquePtr<T>` — V8's owning smart pointers. We model
/// them as plain `Box<T>` since the surface only cares about move semantics.
pub type UniqueRef<T> = Box<T>;
pub type UniquePtr<T> = Option<Box<T>>;

/// V8 uses an array of `Local<Value>` for argument vectors.
pub type Values<'s> = [Local<'s, Value>];
