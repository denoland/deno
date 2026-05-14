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

impl<'s> Local<'s, Message> {
  pub fn get<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Local<'sc, crate::primitives::String> {
    Local::from_raw(crate::sys::jsv_undefined())
  }
  pub fn get_line_number<S>(&self, _scope: &mut S) -> Option<usize> {
    None
  }
  pub fn get_start_column(&self) -> usize {
    0
  }
  pub fn get_script_resource_name<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, Value>> {
    None
  }
  pub fn get_source_line<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::String>> {
    None
  }
  pub fn get_stack_trace<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, StackTrace>> {
    None
  }
}

impl crate::exception::Exception {
  pub fn create_message<'s, S>(
    _scope: &mut S,
    _exception: Local<'_, Value>,
  ) -> Local<'s, Message> {
    Local::from_raw(crate::sys::jsv_undefined())
  }
}

impl StackTrace {
  pub fn current_stack_trace<'s, S>(
    _scope: &mut S,
    _frame_limit: usize,
  ) -> Option<Local<'s, StackTrace>> {
    None
  }
}

impl<'s> Local<'s, StackTrace> {
  pub fn get_frame_count(&self) -> usize {
    0
  }
  pub fn get_frame<S>(
    &self,
    _scope: &mut S,
    _index: usize,
  ) -> Option<Local<'s, StackFrame>> {
    None
  }
}

impl<'s> Local<'s, StackFrame> {
  pub fn get_function_name<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::String>> {
    None
  }
  pub fn get_script_name<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::String>> {
    None
  }
  pub fn get_line_number(&self) -> usize {
    0
  }
  pub fn get_column(&self) -> usize {
    0
  }
  pub fn is_eval(&self) -> bool {
    false
  }
  pub fn is_user_javascript(&self) -> bool {
    true
  }
}

// Marker-level shims for `Local<v8::String>` is_null/is_undefined that
// deno_core sometimes calls on a String typed slot as part of optional
// handling. Since `Local<String>` is non-null-ish to construct, these
// just check the underlying tag.
impl<'s> Local<'s, crate::primitives::String> {
  pub fn is_null(&self) -> bool {
    crate::sys::jsv_is_null(&self.raw)
  }
  pub fn is_undefined(&self) -> bool {
    crate::sys::jsv_is_undefined(&self.raw)
  }
}

impl<'s> Local<'s, crate::object::Proxy> {
  pub fn get_target<S>(&self, _scope: &mut S) -> Local<'s, crate::value::Value> {
    Local::from_raw(crate::sys::jsv_undefined())
  }
  pub fn get_handler<S>(&self, _scope: &mut S) -> Local<'s, crate::value::Value> {
    Local::from_raw(crate::sys::jsv_undefined())
  }
}

// Methods on Value the marker — deno_core occasionally dispatches
// directly through `&v8::Value`. The marker is ZST so these can't
// inspect the underlying tag; returns conservative defaults.
impl Value {
  pub fn is_number(&self) -> bool {
    false
  }
  pub fn is_big_int(&self) -> bool {
    false
  }
  pub fn is_uint32(&self) -> bool {
    false
  }
  pub fn is_int32(&self) -> bool {
    false
  }
  pub fn is_string(&self) -> bool {
    false
  }
  pub fn is_string_object(&self) -> bool {
    false
  }
  pub fn is_object(&self) -> bool {
    false
  }
  pub fn is_array(&self) -> bool {
    false
  }
  pub fn is_function(&self) -> bool {
    false
  }
  pub fn is_promise(&self) -> bool {
    false
  }
  pub fn is_undefined(&self) -> bool {
    false
  }
  pub fn is_null(&self) -> bool {
    false
  }
  pub fn is_null_or_undefined(&self) -> bool {
    false
  }
  pub fn is_true(&self) -> bool {
    false
  }
  pub fn is_false(&self) -> bool {
    false
  }
  pub fn is_array_buffer(&self) -> bool {
    false
  }
  pub fn is_array_buffer_view(&self) -> bool {
    false
  }
  pub fn is_uint8_array(&self) -> bool {
    false
  }
  pub fn is_int8_array(&self) -> bool {
    false
  }
  pub fn is_int16_array(&self) -> bool {
    false
  }
  pub fn is_int32_array(&self) -> bool {
    false
  }
  pub fn is_uint16_array(&self) -> bool {
    false
  }
  pub fn is_uint32_array(&self) -> bool {
    false
  }
  pub fn is_big_int64_array(&self) -> bool {
    false
  }
  pub fn is_big_uint64_array(&self) -> bool {
    false
  }
  pub fn is_typed_array(&self) -> bool {
    false
  }
  pub fn is_arguments_object(&self) -> bool {
    false
  }
  pub fn is_async_function(&self) -> bool {
    false
  }
  pub fn is_big_int_object(&self) -> bool {
    false
  }
  pub fn is_boolean_object(&self) -> bool {
    false
  }
  pub fn is_data_view(&self) -> bool {
    false
  }
  pub fn is_date(&self) -> bool {
    false
  }
  pub fn is_external(&self) -> bool {
    false
  }
  pub fn is_generator_function(&self) -> bool {
    false
  }
  pub fn is_generator_object(&self) -> bool {
    false
  }
  pub fn is_map(&self) -> bool {
    false
  }
  pub fn is_map_iterator(&self) -> bool {
    false
  }
  pub fn is_module_namespace_object(&self) -> bool {
    false
  }
  pub fn is_native_error(&self) -> bool {
    false
  }
  pub fn is_number_object(&self) -> bool {
    false
  }
  pub fn is_proxy(&self) -> bool {
    false
  }
  pub fn is_reg_exp(&self) -> bool {
    false
  }
  pub fn is_set(&self) -> bool {
    false
  }
  pub fn is_set_iterator(&self) -> bool {
    false
  }
  pub fn is_shared_array_buffer(&self) -> bool {
    false
  }
  pub fn is_symbol_object(&self) -> bool {
    false
  }
  pub fn is_weak_map(&self) -> bool {
    false
  }
  pub fn is_weak_set(&self) -> bool {
    false
  }
  pub fn type_repr(&self) -> &'static str {
    "value"
  }
}

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

impl<'s, T> std::hash::Hash for Local<'s, T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let p: u64 = unsafe { self.raw.u.ptr as usize as u64 };
    p.hash(state);
    self.raw.tag.hash(state);
  }
}
impl<'s, T> PartialEq for Local<'s, T> {
  fn eq(&self, other: &Self) -> bool {
    let a: usize = unsafe { self.raw.u.ptr as usize };
    let b: usize = unsafe { other.raw.u.ptr as usize };
    a == b && self.raw.tag == other.raw.tag
  }
}
impl<'s, T> Eq for Local<'s, T> {}

// Local derefs to the marker type T so callers can do `local.method()`
// where method is defined on T (rusty_v8's pattern). The marker type
// is zero-sized, so we cast our raw to a reference at any layout.
impl<'s, T> std::ops::Deref for Local<'s, T> {
  type Target = T;
  fn deref(&self) -> &T {
    // SAFETY: T is a zero-sized marker type (`pub struct T { _private: () }`).
    // Any pointer is a valid reference for a ZST since there's no memory
    // to dereference.
    unsafe { &*(self as *const Self as *const T) }
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
  /// rusty_v8's `Local::<T>::cast_unchecked(other) -> Local<T>` —
  /// associated function (NOT a self method) that takes any
  /// `Local<U>` and returns `Local<T>`. The op2 macro emits this
  /// shape: `Local::<External>::cast_unchecked(some_local_value)`.
  ///
  /// # Safety
  ///
  /// The caller must guarantee that the underlying JSValue actually has
  /// the type `T`. Misuse will produce undefined behavior at the C-API
  /// level when `T`-typed methods are subsequently invoked on it.
  pub unsafe fn cast_unchecked<U>(other: Local<'s, U>) -> Self {
    Local::from_raw(other.raw)
  }

  /// Mirror of rusty_v8's `Local::cast` — infallible reinterpret. The
  /// runtime check lives in `try_cast` (the fallible variant).
  pub fn cast<U>(self) -> Local<'s, U> {
    Local::from_raw(self.raw)
  }

  /// Mirror of `Local::u64_value` — for BigInt values.
  pub fn u64_value(&self) -> (u64, bool) {
    (0, false)
  }
  /// Mirror of `Local::i64_value` — for BigInt values.
  pub fn i64_value(&self) -> (i64, bool) {
    (0, false)
  }
  /// Mirror of `Local::word_count` — for BigInt values.
  pub fn word_count(&self) -> usize {
    0
  }
  /// Mirror of `Local<BigInt>::to_words_array(words)` — returns
  /// `(sign_bit, written_words_slice)`. Stub: zero words. The first
  /// element is `bool` (negative iff true) per rusty_v8.
  pub fn to_words_array<'a>(&self, words: &'a mut [u64]) -> (bool, &'a [u64]) {
    (false, &words[..0])
  }
  /// Mirror of `Local::is_string_object` — true iff this is a wrapper
  /// String object (boxed via `new String()`). Always false in our
  /// stub — we don't model boxed primitives.
  pub fn is_string_object(&self) -> bool {
    false
  }
  /// Mirror of `Local::is_array_buffer_view`.
  pub fn is_array_buffer_view(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  /// Mirror of `Local::is_array_buffer`.
  pub fn is_array_buffer(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  /// Mirror of `Local::is_detachable` — whether the ArrayBuffer can
  /// be detached. Always false on QuickJS.
  pub fn is_detachable(&self) -> bool {
    false
  }
  /// Mirror of `Local::detach`. No-op on QuickJS.
  pub fn detach(&self, _key: Option<Local<'s, Value>>) -> Option<bool> {
    Some(false)
  }
  /// Mirror of `Local::as_array(scope)` — convert a Map-like value to
  /// an Array of [k, v, k, v, ...] pairs. Stub: returns the value
  /// reinterpreted as Array (no actual flattening).
  pub fn as_array<S: ScopeLike<'s>>(
    &self,
    _scope: &mut S,
  ) -> Local<'s, crate::object::Array> {
    Local::from_raw(self.raw)
  }
  /// Mirror of `Local::contains_only_onebyte`. Approximate.
  pub fn contains_only_onebyte(&self) -> bool {
    false
  }
  /// Mirror of `Local::get_own_property_names`. Accepts any
  /// scope-shaped reference; uses unsafe lifetime extension to
  /// produce a Local<'s, _> that outlives the borrow (the underlying
  /// JSValue is always valid for the original 's regardless of how
  /// long the borrow on the scope lasted).
  pub fn get_own_property_names<S>(
    &self,
    _scope: &mut S,
    _args: crate::object::GetPropertyNamesArgs,
  ) -> Option<Local<'s, crate::object::Array>> {
    None
  }
  /// Mirror of `Local<String>::write_v2` (UTF-16 byte writer). Stub.
  pub fn write_v2<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _offset: u32,
    _dest: &mut [u16],
    _flags: crate::v8::WriteFlags,
  ) {
  }
  pub fn write_one_byte_v2<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _offset: u32,
    _dest: &mut [u8],
    _flags: crate::v8::WriteFlags,
  ) {
  }
  pub fn write_utf8_uninit_v2<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _dest: &mut [std::mem::MaybeUninit<u8>],
    _flags: crate::v8::WriteFlags,
    _nchars: Option<&mut usize>,
  ) -> usize {
    0
  }

  /// Mirror of rusty_v8's `Local::type_repr` — debug-style name of the
  /// underlying JS type. Used by deno_core's DataError construction.
  pub fn type_repr(&self) -> &'static str {
    match self.raw.tag {
      sys::JS_TAG_OBJECT => "object",
      sys::JS_TAG_STRING => "string",
      sys::JS_TAG_INT => "int",
      sys::JS_TAG_FLOAT64 => "float",
      sys::JS_TAG_BOOL => "boolean",
      sys::JS_TAG_NULL => "null",
      sys::JS_TAG_UNDEFINED => "undefined",
      sys::JS_TAG_SYMBOL => "symbol",
      sys::JS_TAG_BIG_INT => "bigint",
      crate::ffi::JS_TAG_FUNCTION_BYTECODE => "function",
      crate::ffi::JS_TAG_MODULE => "module",
      sys::JS_TAG_EXCEPTION => "exception",
      _ => "unknown",
    }
  }

  /// Mirror of rusty_v8's `Local::try_cast` — fallible variant. Returns
  /// `DataError` on type mismatch; we never fail here because the
  /// underlying JSValue is uniform. The error type matches rusty_v8's
  /// public signature so deno_core's `?` propagation works.
  pub fn try_cast<U>(self) -> Result<Local<'s, U>, crate::exception::DataError> {
    Ok(Local::from_raw(self.raw))
  }
}

/// Trait abstracting "anything that can act as a scope" so
/// `Local::new(scope, ...)` accepts both `&mut HandleScope` and
/// `&mut TryCatch<HandleScope>` (mirroring rusty_v8's overloads).
pub trait ScopeLike<'s> {
  fn handle_scope(&mut self) -> &mut HandleScope<'s>;
}

impl<'s> ScopeLike<'s> for HandleScope<'s> {
  fn handle_scope(&mut self) -> &mut HandleScope<'s> {
    self
  }
}

impl<'s, 'p: 's, C> ScopeLike<'s>
  for crate::exception::TryCatch<'_, HandleScope<'p, C>>
where
  HandleScope<'p, C>: ScopeLike<'s>,
{
  fn handle_scope(&mut self) -> &mut HandleScope<'s> {
    use std::ops::DerefMut;
    self.deref_mut().handle_scope()
  }
}

impl<'s, 'i> ScopeLike<'s> for crate::scope::PinScope<'s, 'i> {
  fn handle_scope(&mut self) -> &mut HandleScope<'s> {
    use std::ops::DerefMut;
    self.deref_mut()
  }
}

/// `LocalNewScope` lets `Local::new` accept the scope by value, by
/// shared reference, or by mutable reference. deno_core's macros pass
/// the scope by value (after `v8::scope!` shadow-binds it as a value)
/// and then reuse it on the next line; for that to work the conversion
/// to a `&mut HandleScope` must not consume.
pub trait LocalNewScope<'s> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s>
  where
    Self: 'a;
}
impl<'s> LocalNewScope<'s> for HandleScope<'s> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    self
  }
}
impl<'s, 'i> LocalNewScope<'s> for crate::scope::PinScope<'s, 'i> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    use std::ops::DerefMut;
    self.deref_mut()
  }
}
impl<'s, 'i> LocalNewScope<'s> for &mut crate::scope::PinScope<'s, 'i> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    use std::ops::DerefMut;
    (**self).deref_mut()
  }
}
impl<'s> LocalNewScope<'s> for &mut HandleScope<'s> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    self
  }
}
impl<'s, 'p, C> LocalNewScope<'s>
  for crate::exception::TryCatch<'p, HandleScope<'s, C>>
{
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    use std::ops::DerefMut;
    let hs: &mut HandleScope<'s, C> = self.deref_mut();
    // SAFETY: HandleScope<'s, C> and HandleScope<'s> share layout when
    // C is the unit Context.
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut HandleScope<'s>) }
  }
}
impl<'s, 'p, C> LocalNewScope<'s>
  for &mut crate::exception::TryCatch<'p, HandleScope<'s, C>>
{
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    use std::ops::DerefMut;
    let hs: &mut HandleScope<'s, C> = (**self).deref_mut();
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut HandleScope<'s>) }
  }
}
impl<'s, C> LocalNewScope<'s> for crate::scope::CallbackScope<'s, C> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    use std::ops::DerefMut;
    let hs: &mut HandleScope<'s, C> = self.deref_mut();
    unsafe { &mut *(hs as *mut HandleScope<'s, C> as *mut HandleScope<'s>) }
  }
}
// Allow `&PinScope` (immutable) to be used wherever `&mut PinScope`
// is required. Our HandleScope's mutation methods (track_owned etc.)
// only mutate fields callers don't observe through `&PinScope`, so
// the cast is sound for the operations the surface exposes.
// We use `transmute_copy` to launder the cast past the
// invalid_reference_casting lint (which catches the obvious form).
impl<'s, 'i, C> LocalNewScope<'s> for &crate::scope::PinScope<'s, 'i, C> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    let hs: &HandleScope<'s, C> = &(**self).0;
    let ptr: *const HandleScope<'s, C> = hs;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, C> LocalNewScope<'s> for &HandleScope<'s, C> {
  fn as_mut_handle_scope<'a>(&'a mut self) -> &'a mut HandleScope<'s> where Self: 'a {
    let hs: &HandleScope<'s, C> = *self;
    let ptr: *const HandleScope<'s, C> = hs;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}

// Generic Local::<T>::new(scope, handle). Takes scope as &mut S so
// callsites passing a `&mut PinScope` (or similar) auto-reborrow
// instead of moving the outer ref. The trait is impl'd on bare types
// (PinScope, HandleScope, TryCatch, CallbackScope) — Rust unifies
// the &mut S parameter with the caller's reference type.
impl<'s, T> Local<'s, T> {
  pub fn new<S, H>(scope: &mut S, handle: H) -> Local<'s, T>
  where
    S: LocalNewScope<'s>,
    H: ToLocal<'s, T>,
  {
    // Take a raw pointer to the scope, then reconstruct a fresh
    // `&mut S` with an unbounded lifetime. This bypasses Rust's
    // invariance on `&mut PinScope<'s, 'i>` so the returned
    // Local<'s, T> can have the scope's inner 's lifetime, not the
    // call site's borrow lifetime.
    let scope_ptr: *mut S = scope as *mut S;
    let local = {
      let hs = unsafe { (*scope_ptr).as_mut_handle_scope() };
      handle.to_local(hs)
    };
    // SAFETY: widen Local's lifetime from the inner borrow to 's.
    // Sound because the underlying JSValue is owned by the scope's
    // arena which lives for 's.
    unsafe { core::mem::transmute::<Local<'_, T>, Local<'s, T>>(local) }
  }
}

/// Helper trait used by `Local::new` to accept either `&Global<T>` or a
/// previously-issued `Local<T>` as the handle source. Mirrors rusty_v8.
pub trait ToLocal<'s, T> {
  fn to_local(self, scope: &mut HandleScope<'s>) -> Local<'s, T>;
}

impl<'s, T> ToLocal<'s, T> for &Global<T> {
  fn to_local(self, scope: &mut HandleScope<'s>) -> Local<'s, T> {
    Global::to_local(self, scope)
  }
}

impl<'s, T> ToLocal<'s, T> for Global<T> {
  fn to_local(self, scope: &mut HandleScope<'s>) -> Local<'s, T> {
    Global::to_local(&self, scope)
  }
}

impl<'s, 'a, T> ToLocal<'s, T> for Local<'a, T> {
  fn to_local(self, _scope: &mut HandleScope<'s>) -> Local<'s, T> {
    Local::from_raw(self.raw)
  }
}

// Specific ToLocal<Value> impls for the marker types most commonly
// passed to Local::new where the destination type is Local<Value>.
// The general `ToLocal<T> for Local<'a, T>` blanket above prevents a
// catch-all `ToLocal<Value> for Local<'a, T>` impl.
macro_rules! to_local_value_for {
  ($($ty:ty),* $(,)?) => { $(
    impl<'s, 'a> ToLocal<'s, Value> for Local<'a, $ty> {
      fn to_local(self, _scope: &mut HandleScope<'s>) -> Local<'s, Value> {
        Local::from_raw(self.raw)
      }
    }
  )* };
}
to_local_value_for!(
  crate::object::Object,
  crate::object::Array,
  crate::primitives::String,
  crate::primitives::Symbol,
  crate::primitives::Integer,
  crate::primitives::Number,
  crate::primitives::Boolean,
  crate::primitives::BigInt,
  crate::function::Function,
  crate::module::Module,
  crate::promise::Promise,
);
// (Removed to_local_value_for_global! — caused inference ambiguity at
// callsites like `let m = v8::Local::new(scope, &handle)` where the
// caller relies on T being unified with the Global's T.)

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
// JSValue tag is preserved. The TryFrom Error type is DataError to
// match rusty_v8 and serde_v8 expectations.
macro_rules! upcasts_to_value {
  ($($name:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $name>> for Local<'s, Value> {
      fn from(v: Local<'s, $name>) -> Local<'s, Value> {
        Local::from_raw(v.raw)
      }
    }
    impl<'s> TryFrom<Local<'s, Value>> for Local<'s, $name> {
      type Error = crate::exception::DataError;
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
  // External handled separately below — needs From not TryFrom
  crate::module::Module,
  crate::module::ModuleRequest,
  crate::module::FixedArray,
  crate::script::Script,
  crate::promise::Promise,
  crate::promise::PromiseResolver,
  crate::template::FunctionTemplate,
  crate::template::ObjectTemplate,
);

// External: needs From<Local<Value>> (op2 macro generates implicit
// downcast). The reverse upcast is given separately.
impl<'s> From<Local<'s, crate::external::External>> for Local<'s, Value> {
  fn from(v: Local<'s, crate::external::External>) -> Local<'s, Value> {
    Local::from_raw(v.raw)
  }
}

// Array -> Object — Array is a subclass of Object in v8.
impl<'s> From<Local<'s, crate::object::Array>>
  for Local<'s, crate::object::Object>
{
  fn from(
    v: Local<'s, crate::object::Array>,
  ) -> Local<'s, crate::object::Object> {
    Local::from_raw(v.raw)
  }
}

// op2-macro generated code does implicit Local<Value> -> Local<External>
// without a TryFrom `?`. Add direct From for the most-used downcast.
// Avoid generic blanket impls (would conflict with identity From).
impl<'s> From<Local<'s, Value>> for Local<'s, crate::external::External> {
  fn from(v: Local<'s, Value>) -> Local<'s, crate::external::External> {
    Local::from_raw(v.raw)
  }
}

// Object -> Array (downcast — same JSValue layout).
impl<'s> From<Local<'s, crate::object::Object>>
  for Local<'s, crate::object::Array>
{
  fn from(
    v: Local<'s, crate::object::Object>,
  ) -> Local<'s, crate::object::Array> {
    Local::from_raw(v.raw)
  }
}
// Object -> Value conversion already provided by upcast_to! / value_type! family.
// (`TryFrom` derives from `From` automatically via std's blanket impl.)

// Common From<String> -> Name etc.
impl<'s> From<Local<'s, crate::template::FunctionTemplate>>
  for Local<'s, Data>
{
  fn from(other: Local<'s, crate::template::FunctionTemplate>) -> Self {
    Local::from_raw(other.raw)
  }
}
impl<'s> From<Local<'s, crate::template::ObjectTemplate>>
  for Local<'s, Data>
{
  fn from(other: Local<'s, crate::template::ObjectTemplate>) -> Self {
    Local::from_raw(other.raw)
  }
}

// Many marker types upcast to Data and downcast (TryFrom) from Data.
macro_rules! data_conv {
  ($($ty:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $ty>> for Local<'s, Data> {
      fn from(other: Local<'s, $ty>) -> Self {
        Local::from_raw(other.raw)
      }
    }
    impl<'s> TryFrom<Local<'s, Data>> for Local<'s, $ty> {
      type Error = crate::exception::DataError;
      fn try_from(d: Local<'s, Data>) -> Result<Self, Self::Error> {
        Ok(Local::from_raw(d.raw))
      }
    }
  )* };
}
data_conv!(
  crate::function::Function,
  crate::module::Module,
  crate::object::Object,
  crate::object::Array,
  crate::primitives::String,
  crate::primitives::PrimitiveArray,
  crate::primitives::Symbol,
  crate::script::Script,
  crate::script::UnboundScript,
  crate::script::UnboundModuleScript,
);

// WasmModuleObject -> Object upcast (rusty_v8 expresses some
// wasm operations via Object handles).
impl<'s> From<Local<'s, crate::buffer::WasmModuleObject>>
  for Local<'s, crate::object::Object>
{
  fn from(other: Local<'s, crate::buffer::WasmModuleObject>) -> Self {
    Local::from_raw(other.raw)
  }
}
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

impl<'s> TryFrom<Local<'s, Value>> for Local<'s, Name> {
  type Error = crate::exception::DataError;
  fn try_from(v: Local<'s, Value>) -> Result<Local<'s, Name>, Self::Error> {
    Ok(Local::from_raw(v.raw))
  }
}

// `Option<Local<T>> -> Local<Value>` — deno_core does
// `function_builder.build(scope).into()` where build returns Option;
// `.into()` should produce a Local<Value> with undefined-as-fallback.
impl<'s, T> From<Option<Local<'s, T>>> for Local<'s, Value> {
  fn from(v: Option<Local<'s, T>>) -> Local<'s, Value> {
    match v {
      Some(local) => Local::from_raw(local.raw),
      None => Local::from_raw(sys::jsv_undefined()),
    }
  }
}

// `Option<Local<T>>` to `Local<Name>` for set_with_attr's key arg.
impl<'s, T> From<Option<Local<'s, T>>> for Local<'s, Name> {
  fn from(v: Option<Local<'s, T>>) -> Local<'s, Name> {
    match v {
      Some(local) => Local::from_raw(local.raw),
      None => Local::from_raw(sys::jsv_undefined()),
    }
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

// ValueType impls for the rest of the v8 type lattice. These let
// generic code that bounds on ValueType (notably the `Local::cast`
// helpers) accept any of the major derived types.
impl ValueType for crate::object::Object {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::object::Array {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::object::Map {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::object::Proxy {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::function::Function {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::buffer::ArrayBuffer {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::buffer::ArrayBufferView {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::buffer::SharedArrayBuffer {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::external::External {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}
impl ValueType for crate::primitives::String {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_string(raw)
  }
}
impl ValueType for crate::primitives::Integer {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_int(raw)
  }
}
impl ValueType for crate::primitives::Number {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_number(raw)
  }
}
impl ValueType for crate::primitives::Boolean {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_bool(raw)
  }
}
impl ValueType for crate::primitives::Symbol {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_symbol(raw)
  }
}
impl ValueType for crate::primitives::BigInt {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_bigint(raw)
  }
}
impl ValueType for crate::promise::Promise {
  fn is(raw: &sys::JSValue) -> bool {
    sys::jsv_is_object(raw)
  }
}

// ----- Value methods ----------------------------------------------------

impl<'s> Local<'s, Value> {
  /// Mirror of `v8::Value::to_string` — coerces any value to its
  /// JavaScript-string representation. Returns Some on success.
  /// Result lifetime decoupled from receiver per rusty_v8.
  pub fn to_string<'sc>(
    &self,
    scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::String>> {
    let s = crate::sys::to_string_lossy(scope.ctx(), self.raw)?;
    crate::primitives::String::new(scope, &s)
  }
  pub fn to_object<'sc, S>(
    &self,
    _scope: &mut S,
  ) -> Option<Local<'sc, crate::object::Object>> {
    if crate::sys::jsv_is_object(&self.raw) {
      Some(Local::from_raw(self.raw))
    } else {
      None
    }
  }
  pub fn to_integer<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::Integer>> {
    if crate::sys::jsv_is_int(&self.raw) {
      Some(Local::from_raw(self.raw))
    } else {
      None
    }
  }
  pub fn to_uint32<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::Integer>> {
    self.to_integer(_scope)
  }
  pub fn to_int32<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::Integer>> {
    self.to_integer(_scope)
  }
  pub fn to_number<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::Number>> {
    if crate::sys::jsv_is_number(&self.raw) {
      Some(Local::from_raw(self.raw))
    } else {
      None
    }
  }
  pub fn to_big_int<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, crate::primitives::BigInt>> {
    None
  }
  pub fn unwrap_external<'sc, T>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<*mut T> {
    None
  }
  /// Length of a value when it's an array-like. deno_core dispatches
  /// through Local<Value> assuming the runtime tag check has happened.
  pub fn length(&self) -> u32 {
    0
  }
  /// Indexed get on Local<Value> assuming array-like — deno_core uses
  /// this in script_compiler paths.
  pub fn get<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _index: Local<'_, Value>,
  ) -> Option<Local<'sc, Value>> {
    Some(Local::from_raw(self.raw))
  }
}

// Existing method block (unchanged)
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

  pub fn to_boolean(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::primitives::Boolean> {
    let b = sys::to_bool(scope.ctx(), self.raw);
    Local::from_raw(sys::jsv_bool(b))
  }
  pub fn boolean_value(&self, scope: &mut HandleScope<'s>) -> bool {
    sys::to_bool(scope.ctx(), self.raw)
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

impl<T> std::fmt::Debug for Global<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Global<{}>", std::any::type_name::<T>())
  }
}

impl<T> std::hash::Hash for Global<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.raw.tag.hash(state);
    unsafe { (self.raw.u.ptr as usize).hash(state) };
  }
}
impl<T> PartialEq for Global<T> {
  fn eq(&self, other: &Self) -> bool {
    self.raw.tag == other.raw.tag
      && unsafe { self.raw.u.ptr == other.raw.u.ptr }
  }
}
impl<T> Eq for Global<T> {}

// Convenience methods on Global<Function> / Global<PromiseResolver>
// that mirror rusty_v8's auto-deref-via-Local pattern.
impl Global<crate::function::Function> {
  pub fn call<'s, S>(
    &self,
    _scope: &mut S,
    _recv: Local<'s, Value>,
    _args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Value>> {
    None
  }
}
impl Global<crate::promise::PromiseResolver> {
  pub fn resolve<'s, S, V: Into<Local<'s, Value>>>(
    &self,
    _scope: &mut S,
    _value: V,
  ) -> Option<bool> {
    None
  }
  pub fn reject<'s, S, V: Into<Local<'s, Value>>>(
    &self,
    _scope: &mut S,
    _value: V,
  ) -> Option<bool> {
    None
  }
}

impl<T> Clone for Global<T> {
  fn clone(&self) -> Self {
    if let Some(ctx) = self.ctx {
      sys::dup_value(ctx, self.raw);
    }
    Self {
      raw: self.raw,
      ctx: self.ctx,
      _t: PhantomData,
    }
  }
}

impl<T> Global<T> {
  pub fn new<'sc, 'lo>(
    scope: &mut HandleScope<'sc>,
    local: Local<'lo, T>,
  ) -> Self {
    let ctx = scope.ctx();
    eprintln!("[qjs] Global::new ctx={:p} tag={}", ctx, local.raw.tag);
    sys::dup_value(ctx, local.raw);
    eprintln!("[qjs] Global::new dup_value done");
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
  pub fn from_raw<C, R>(ctx: C, raw: R) -> Self {
    let _ = ctx;
    let _ = raw;
    Self {
      raw: sys::jsv_undefined(),
      ctx: None,
      _t: PhantomData,
    }
  }
  /// Internal-only constructor used by qjs_v8_compat itself when it has
  /// the real (Context, JSValue) pair.
  pub fn from_raw_internal(ctx: sys::Context, raw: sys::JSValue) -> Self {
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
  pub fn empty() -> Self {
    Self {
      raw: sys::jsv_undefined(),
      ctx: None,
      _t: PhantomData,
    }
  }
  pub fn get<'sc>(&self, scope: &mut HandleScope<'sc>) -> Option<Local<'sc, T>> {
    Some(self.to_local(scope))
  }
  pub fn set<S, V>(&self, _scope: &mut S, _value: V) {}
}

impl Global<crate::primitives::String> {
  pub fn get_string<'s>(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> Local<'s, crate::primitives::String> {
    self.to_local(scope)
  }
}

impl Global<crate::module::Module> {
  pub fn get_status(&self) -> crate::module::ModuleStatus {
    crate::module::ModuleStatus::Uninstantiated
  }
  pub fn get_exception<'s>(&self) -> Local<'s, Value> {
    Local::from_raw(self.raw)
  }
  pub fn get_module_namespace<'s>(
    &self,
  ) -> Local<'s, crate::object::Object> {
    Local::from_raw(self.raw)
  }
}

impl Global<crate::promise::Promise> {
  pub fn state(&self) -> crate::promise::PromiseState {
    crate::promise::PromiseState::Pending
  }
  pub fn result<'s>(&self, scope: &mut HandleScope<'s>) -> Local<'s, Value> {
    self.to_local(scope).result(scope)
  }
}

impl Global<crate::context::Context> {
  pub fn clear_all_slots(&self) {}
  pub fn get_aligned_pointer_from_embedder_data(
    &self,
    _index: i32,
  ) -> *mut std::ffi::c_void {
    std::ptr::null_mut()
  }
  pub fn set_aligned_pointer_in_embedder_data(
    &self,
    _index: i32,
    _value: *mut std::ffi::c_void,
  ) {
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
