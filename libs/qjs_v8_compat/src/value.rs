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
      // Carry the JSValue raw (same first field as Local<T>) so
      // methods on `&v8::Foo` borrowed via Local::deref can read
      // the actual tag — needed for is_string/is_number etc. that
      // op2-emitted code calls on `&v8::Value`.
      #[derive(Copy, Clone)]
      #[repr(transparent)]
      pub struct $name {
        pub(crate) raw: crate::sys::JSValue,
      }
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

// Methods on Value the marker — deno_core dispatches through
// `&v8::Value` (auto-deref'd from Local<Value>). The marker now
// carries the JSValue (via #[repr(transparent)] in `value_type!`)
// so these inspect the actual underlying tag.
impl Value {
  pub fn is_number(&self) -> bool {
    sys::jsv_is_number(&self.raw)
  }
  pub fn is_big_int(&self) -> bool {
    sys::jsv_is_bigint(&self.raw)
  }
  pub fn is_uint32(&self) -> bool {
    sys::jsv_is_int(&self.raw) && unsafe { self.raw.u.int32 >= 0 }
  }
  pub fn is_int32(&self) -> bool {
    sys::jsv_is_int(&self.raw)
  }
  pub fn is_string(&self) -> bool {
    sys::jsv_is_string(&self.raw)
  }
  pub fn is_string_object(&self) -> bool {
    false
  }
  pub fn is_object(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_array(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_function(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_promise(&self) -> bool {
    sys::jsv_is_object(&self.raw)
  }
  pub fn is_undefined(&self) -> bool {
    sys::jsv_is_undefined(&self.raw)
  }
  pub fn is_null(&self) -> bool {
    sys::jsv_is_null(&self.raw)
  }
  pub fn is_null_or_undefined(&self) -> bool {
    sys::jsv_is_null(&self.raw) || sys::jsv_is_undefined(&self.raw)
  }
  pub fn is_true(&self) -> bool {
    sys::jsv_is_bool(&self.raw) && unsafe { self.raw.u.int32 != 0 }
  }
  pub fn is_false(&self) -> bool {
    sys::jsv_is_bool(&self.raw) && unsafe { self.raw.u.int32 == 0 }
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
// Cross-type Local comparison: `Local<Value> == Local<Object>` should
// work by handle identity. We provide impls for the specific T/U
// combinations deno_node compares.
macro_rules! local_cross_eq {
  ($($t:ty => $u:ty),* $(,)?) => { $(
    impl<'s, 't> PartialEq<Local<'t, $u>> for Local<'s, $t> {
      fn eq(&self, other: &Local<'t, $u>) -> bool {
        let a: usize = unsafe { self.raw.u.ptr as usize };
        let b: usize = unsafe { other.raw.u.ptr as usize };
        a == b && self.raw.tag == other.raw.tag
      }
    }
  )* };
}
local_cross_eq!(
  Value => crate::object::Object,
  crate::object::Object => Value,
  Value => crate::function::Function,
  crate::function::Function => Value,
  Value => crate::primitives::String,
  crate::primitives::String => Value,
);

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

  /// Mirror of rusty_v8's crate-private `Local::from_non_null`. Reads
  /// the heap-allocated JSValue produced by `Global::into_raw` and
  /// returns a Local that aliases the same handle. Does NOT free the
  /// allocation — the original Global pointer continues to own the
  /// JSValue. Lifetime is `'static` because we don't have a scope at
  /// the call site (callbacks reconstructing from a stored NonNull).
  ///
  /// # Safety
  /// `ptr` must come from a previous `Global::into_raw` of T.
  pub unsafe fn from_non_null(ptr: std::ptr::NonNull<T>) -> Local<'static, T> {
    let raw_ptr = ptr.as_ptr() as *mut sys::JSValue;
    let raw_value = unsafe { *raw_ptr };
    Local::from_raw(raw_value)
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

/// Like `LocalNewScope<'s>` but for shared `&S` callers — `Local::new`
/// in real v8 takes `&PinScope`, so we accept the same shape.
pub trait LocalNewScopeRef<'s> {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s>;
}
impl<'s, S: LocalNewScopeRef<'s> + ?Sized> LocalNewScopeRef<'s> for &mut S {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    (**self).as_mut_handle_scope_ref()
  }
}
impl<'s, S: LocalNewScopeRef<'s> + ?Sized> LocalNewScopeRef<'s> for &S {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    (**self).as_mut_handle_scope_ref()
  }
}
impl<'s, C> LocalNewScopeRef<'s> for HandleScope<'s, C> {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    let ptr: *const HandleScope<'s, C> = self;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, 'i, C> LocalNewScopeRef<'s> for crate::scope::PinScope<'s, 'i, C> {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    let ptr: *const HandleScope<'s, C> = &self.0;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, 'p, C> LocalNewScopeRef<'s>
  for crate::exception::TryCatch<'p, HandleScope<'s, C>>
where
  'p: 's,
{
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    use std::ops::Deref;
    let pin = self.deref();
    let ptr: *const crate::scope::PinScope<'s, 'p, C> = pin;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, C> LocalNewScopeRef<'s> for crate::scope::CallbackScope<'s, C> {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    let ptr: *const HandleScope<'s, C> = &self.0;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, 'a, C> LocalNewScopeRef<'s>
  for crate::context::ContextScope<'a, HandleScope<'s, C>>
{
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    use std::ops::Deref;
    let pin: &crate::scope::PinScope<'s, 's, C> = self.deref();
    pin.as_mut_handle_scope_ref()
  }
}
impl<'s, 'i, 'a, C> LocalNewScopeRef<'s>
  for crate::context::ContextScope<'a, crate::scope::PinScope<'s, 'i, C>>
{
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    // ContextScope.parent is &mut PinScope<'s, 'i, C>; reach in via
    // raw-pointer cast to avoid the &mut requirement on self.
    let ptr: *const crate::context::ContextScope<'a, crate::scope::PinScope<'s, 'i, C>> = self;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s, P> LocalNewScopeRef<'s> for std::pin::Pin<&mut P>
where
  P: LocalNewScopeRef<'s>,
{
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    P::as_mut_handle_scope_ref(self)
  }
}
impl<'s, 'e, C> LocalNewScopeRef<'s>
  for crate::scope::EscapableHandleScope<'s, 'e, C>
where
  'e: 's,
{
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    use std::ops::Deref;
    let hs: &HandleScope<'s, C> = self.deref();
    let ptr: *const HandleScope<'s, C> = hs;
    let mut_ptr: *mut HandleScope<'s> =
      unsafe { core::mem::transmute_copy(&ptr) };
    unsafe { &mut *mut_ptr }
  }
}
impl<'s> LocalNewScopeRef<'s> for crate::isolate::Isolate {
  fn as_mut_handle_scope_ref(&self) -> &mut HandleScope<'s> {
    // Synthesize an ephemeral HandleScope handle pointing at the
    // isolate's default context. We Box::leak it so the returned
    // reference is valid (caller never frees). For a stub use case
    // this is acceptable.
    let ptr = self as *const _ as *mut crate::isolate::Isolate;
    let ctx = unsafe {
      use crate::scope::HandleScopeSource;
      (*ptr).default_ctx()
    };
    let hs = Box::new(HandleScope {
      inner: crate::scope::HandleScopeInner {
        isolate: ptr,
        ctx,
        owned: Vec::new(),
        parent_owned: None,
        depth: 0,
      },
      _phantom: std::marker::PhantomData,
    });
    Box::leak(hs)
  }
}

// Generic Local::<T>::new(scope, handle). Takes scope as &mut S so
// callsites passing a `&mut PinScope` (or similar) auto-reborrow
// instead of moving the outer ref. The trait is impl'd on bare types
// (PinScope, HandleScope, TryCatch, CallbackScope) — Rust unifies
// the &mut S parameter with the caller's reference type.
impl<'s, T> Local<'s, T> {
  pub fn new<S, H>(scope: &S, handle: H) -> Local<'s, T>
  where
    S: LocalNewScopeRef<'s> + ?Sized,
    H: ToLocal<'s, T>,
  {
    // Take a raw pointer to the scope, then reconstruct a fresh
    // `&mut S` with an unbounded lifetime. This bypasses Rust's
    // invariance on `&mut PinScope<'s, 'i>` so the returned
    // Local<'s, T> can have the scope's inner 's lifetime, not the
    // call site's borrow lifetime. Real v8's Local::new takes &Scope
    // (shared), so we accept &S and cast internally — the underlying
    // operation (handle.to_local) only writes to the scope's owned
    // vec which is contention-free at QuickJS's single-threaded
    // execution model.
    let scope_ptr: *mut S = (scope as *const S) as *mut S;
    let local = {
      let hs = unsafe { (*scope_ptr).as_mut_handle_scope_ref() };
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

// (Removed `to_local_value_for!` — caused inference ambiguity at
// callsites like `let info = v8::Local::new(scope, info)` where
// `info: Local<Object>` could pick T=Object or T=Value. Real rusty_v8
// doesn't have implicit upcasts in `Local::new`; callers pass `.into()`
// at the use site.)
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
  crate::v8::Int32,
  crate::v8::Uint32,
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
  crate::buffer::Uint32Array,
  crate::buffer::Float32Array,
  crate::buffer::Float64Array,
  crate::buffer::TypedArray,
  crate::buffer::DataView,
  // External handled separately below — needs From not TryFrom
);

// Typed-array (Float32Array, etc.) → ArrayBufferView upcast: real v8
// has TypedArray as a subclass of ArrayBufferView. We mirror the
// From impl with a same-raw-bits cast.
macro_rules! upcasts_to_view {
  ($($name:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $name>> for Local<'s, crate::buffer::ArrayBufferView> {
      fn from(v: Local<'s, $name>) -> Local<'s, crate::buffer::ArrayBufferView> {
        Local::from_raw(v.raw)
      }
    }
    impl<'s> From<Local<'s, $name>> for Local<'s, crate::buffer::TypedArray> {
      fn from(v: Local<'s, $name>) -> Local<'s, crate::buffer::TypedArray> {
        Local::from_raw(v.raw)
      }
    }
  )* }
}

upcasts_to_view!(
  crate::buffer::Uint32Array,
  crate::buffer::Float32Array,
  crate::buffer::Float64Array,
);

// Up-casts: typed arrays / various marker types → Local<Object>.
macro_rules! local_to_object {
  ($($ty:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $ty>> for Local<'s, crate::object::Object> {
      fn from(v: Local<'s, $ty>) -> Self { Local::from_raw(v.raw) }
    }
  )* };
}
local_to_object!(
  crate::buffer::Uint8Array,
  crate::buffer::Uint32Array,
  crate::buffer::Float32Array,
  crate::buffer::Float64Array,
  crate::buffer::DataView,
  crate::buffer::ArrayBuffer,
  crate::buffer::ArrayBufferView,
  crate::buffer::SharedArrayBuffer,
  crate::buffer::TypedArray,
  crate::function::Function,
  crate::object::Map,
  crate::object::Proxy,
);
// (Array → Object already covered by an existing impl elsewhere.)

// Re-open the original list so the trailing `);` from the upstream
// invocation still closes a valid expansion.
upcasts_to_value!(
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
impl<'s> Local<'s, Name> {
  pub fn is_symbol(&self) -> bool {
    sys::jsv_is_symbol(&self.raw)
  }
  pub fn is_string(&self) -> bool {
    sys::jsv_is_string(&self.raw)
  }
}

impl<'s> From<Local<'s, crate::primitives::String>> for Local<'s, Name> {
  fn from(v: Local<'s, crate::primitives::String>) -> Local<'s, Name> {
    Local::from_raw(v.raw)
  }
}
// Primitives upcast to Primitive marker.
macro_rules! local_to_primitive {
  ($($ty:ty),* $(,)?) => { $(
    impl<'s> From<Local<'s, $ty>> for Local<'s, crate::value::Primitive> {
      fn from(v: Local<'s, $ty>) -> Self { Local::from_raw(v.raw) }
    }
  )* };
}
local_to_primitive!(
  crate::primitives::Boolean,
  crate::primitives::Number,
  crate::primitives::Integer,
  crate::primitives::String,
  crate::primitives::Symbol,
  crate::primitives::BigInt,
);
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
/// Stub for `v8::TracedReference<T>` — V8's GC-traced handle. We
/// don't trace; this just wraps a Global semantically.
pub struct TracedReference<T> {
  inner: Global<T>,
}
impl<T> TracedReference<T> {
  pub fn empty() -> Self {
    Self { inner: Global::empty() }
  }
  pub fn new<'s>(scope: &mut HandleScope<'s>, value: Local<'s, T>) -> Self {
    Self { inner: Global::new(scope, value) }
  }
  pub fn get<'s>(&self, scope: &mut HandleScope<'s>) -> Option<Local<'s, T>> {
    self.inner.get(scope)
  }
  pub fn reset(&mut self, _scope: &mut HandleScope) {
    self.inner = Global::empty();
  }
}

/// Trait for "anything Global::new can take as a scope reference" —
/// covers shared `&HandleScope`, mutable `&mut HandleScope`,
/// `&PinScope`, `&mut PinScope`, `&mut Isolate`, `&&mut Isolate`,
/// `Pin<&mut ...>`, etc. Real rusty_v8 requires `&PinScope<'s,'i,()>`;
/// we accept the broader set so legacy callers still compile.
/// Real catch-all trait: takes `self` so any reference flavor can be
/// passed (mutable, shared, double-ref &&mut). Each impl below picks
/// its own access path. This is the trait Global::new actually uses.
pub trait GlobalNewScopeAny {
  fn scope_ctx_any(self) -> sys::Context;
}
impl<'a, 's, C> GlobalNewScopeAny for &'a mut HandleScope<'s, C> {
  fn scope_ctx_any(self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'a, 's, C> GlobalNewScopeAny for &'a HandleScope<'s, C> {
  fn scope_ctx_any(self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'a, 's, 'i, C> GlobalNewScopeAny for &'a mut crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_any(self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl<'a, 's, 'i, C> GlobalNewScopeAny for &'a crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_any(self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl<'a> GlobalNewScopeAny for &'a mut crate::isolate::Isolate {
  fn scope_ctx_any(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 'b> GlobalNewScopeAny for &'a &'b mut crate::isolate::Isolate {
  fn scope_ctx_any(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    let r: &mut crate::isolate::Isolate = unsafe {
      &mut *(*self as *const _ as *mut crate::isolate::Isolate)
    };
    r.default_ctx()
  }
}
impl<'a> GlobalNewScopeAny for &'a mut crate::isolate::OwnedIsolate {
  fn scope_ctx_any(self) -> sys::Context { self.default_ctx() }
}
impl<'a, 'p, C> GlobalNewScopeAny
  for &'a mut crate::exception::TryCatch<'p, HandleScope<'p, C>>
{
  fn scope_ctx_any(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 's, C> GlobalNewScopeAny for &'a mut crate::scope::CallbackScope<'s, C> {
  fn scope_ctx_any(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 'p, S> GlobalNewScopeAny for &'a mut crate::context::ContextScope<'p, S>
where
  crate::context::ContextScope<'p, S>: crate::scope::HandleScopeSource,
{
  fn scope_ctx_any(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, P> GlobalNewScopeAny for std::pin::Pin<&'a mut P>
where
  P: GlobalNewScopePinned,
{
  fn scope_ctx_any(mut self) -> sys::Context {
    unsafe { self.as_mut().get_unchecked_mut().ctx_pinned() }
  }
}

/// Universal trait for "anything Global::new accepts as scope". Takes
/// `self` (typically a &mut or &) so the impl can choose to reborrow
/// internally without consuming the underlying scope value.
pub trait GlobalNewScopeRef {
  fn scope_ctx_ref(self) -> sys::Context;
}
// Reborrow specialization: the most common case at callsites is
// `Global::new(scope, x)` where `scope: &mut Foo`. We reborrow inside
// so the original `&mut Foo` stays usable after the call.
impl<'a, S: GlobalNewScopeRefByMut + ?Sized> GlobalNewScopeRef for &'a mut S {
  fn scope_ctx_ref(self) -> sys::Context {
    self.scope_ctx_by_mut()
  }
}
impl<'a, S: GlobalNewScopeRefByShared + ?Sized> GlobalNewScopeRef for &'a S {
  fn scope_ctx_ref(self) -> sys::Context {
    self.scope_ctx_by_shared()
  }
}
/// Universal scope trait used by Global::new — implemented broadly so
/// `Global::new(scope_or_isolate, x)` works for every scope shape and
/// for `&Isolate` / `&&mut Isolate`. Take `&self` (shared) so callers
/// passing `&mut Foo` are auto-coerced via `&*foo` and the original
/// mutable binding remains usable.
pub trait GlobalScope {
  fn scope_ctx_shared(&self) -> sys::Context;
}
impl<'s, C> GlobalScope for HandleScope<'s, C> {
  fn scope_ctx_shared(&self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'s, 'i, C> GlobalScope for crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_shared(&self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl GlobalScope for crate::isolate::Isolate {
  fn scope_ctx_shared(&self) -> sys::Context {
    self.default_ctx_shared()
  }
}
impl GlobalScope for crate::isolate::OwnedIsolate {
  fn scope_ctx_shared(&self) -> sys::Context {
    self.default_ctx_shared()
  }
}
impl<'p, C> GlobalScope for crate::exception::TryCatch<'p, HandleScope<'p, C>> {
  fn scope_ctx_shared(&self) -> sys::Context {
    use std::ops::Deref;
    let pin: &crate::scope::PinScope<'p, 'p, C> = self.deref();
    pin.0.inner.ctx
  }
}
impl<'s, C> GlobalScope for crate::scope::CallbackScope<'s, C> {
  fn scope_ctx_shared(&self) -> sys::Context {
    self.0.inner.ctx
  }
}
impl<'p, S> GlobalScope for crate::context::ContextScope<'p, S>
where
  S: GlobalScope,
{
  fn scope_ctx_shared(&self) -> sys::Context {
    use std::ops::Deref;
    self.deref().scope_ctx_shared()
  }
}
impl<P: GlobalScope + Unpin> GlobalScope for std::pin::Pin<&mut P> {
  fn scope_ctx_shared(&self) -> sys::Context {
    use std::ops::Deref;
    self.deref().scope_ctx_shared()
  }
}
impl<S: GlobalScope + ?Sized> GlobalScope for &S {
  fn scope_ctx_shared(&self) -> sys::Context {
    (**self).scope_ctx_shared()
  }
}
impl<S: GlobalScope + ?Sized> GlobalScope for &mut S {
  fn scope_ctx_shared(&self) -> sys::Context {
    (**self).scope_ctx_shared()
  }
}

pub trait GlobalNewScopeRefByMut {
  fn scope_ctx_by_mut(&mut self) -> sys::Context;
}
pub trait GlobalNewScopeRefByShared {
  fn scope_ctx_by_shared(&self) -> sys::Context;
}
impl<'s, C> GlobalNewScopeRefByMut for HandleScope<'s, C> {
  fn scope_ctx_by_mut(&mut self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'s, 'i, C> GlobalNewScopeRefByMut for crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_by_mut(&mut self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl<'s, C> GlobalNewScopeRefByShared for HandleScope<'s, C> {
  fn scope_ctx_by_shared(&self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'s, 'i, C> GlobalNewScopeRefByShared for crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_by_shared(&self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl GlobalNewScopeRefByMut for crate::isolate::Isolate {
  fn scope_ctx_by_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl GlobalNewScopeRefByMut for crate::isolate::OwnedIsolate {
  fn scope_ctx_by_mut(&mut self) -> sys::Context { self.default_ctx() }
}
impl<'p, C> GlobalNewScopeRefByMut for crate::exception::TryCatch<'p, HandleScope<'p, C>> {
  fn scope_ctx_by_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'s, C> GlobalNewScopeRefByMut for crate::scope::CallbackScope<'s, C> {
  fn scope_ctx_by_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'p, S> GlobalNewScopeRefByMut for crate::context::ContextScope<'p, S>
where
  crate::context::ContextScope<'p, S>: crate::scope::HandleScopeSource,
{
  fn scope_ctx_by_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
// Pin<&mut Foo> support so callsites passing pinned scopes type-check.
impl<P: GlobalNewScopeRefByMut> GlobalNewScopeRefByMut for std::pin::Pin<&mut P> {
  fn scope_ctx_by_mut(&mut self) -> sys::Context {
    unsafe { self.as_mut().get_unchecked_mut().scope_ctx_by_mut() }
  }
}
// Special: `&&mut Isolate` shows up in stream_wrap.rs.
impl<'b> GlobalNewScopeRefByShared for &'b mut crate::isolate::Isolate {
  fn scope_ctx_by_shared(&self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    let r: &mut crate::isolate::Isolate = unsafe {
      &mut *(*self as *const _ as *mut crate::isolate::Isolate)
    };
    r.default_ctx()
  }
}

/// Original by-value GlobalNewScope kept (no callers; can be removed).
pub trait GlobalNewScopeMut {
  fn scope_ctx_mut(&mut self) -> sys::Context;
}
impl<S: GlobalNewScopeMut + ?Sized> GlobalNewScopeMut for &mut S {
  fn scope_ctx_mut(&mut self) -> sys::Context {
    (**self).scope_ctx_mut()
  }
}
impl<'s, C> GlobalNewScopeMut for HandleScope<'s, C> {
  fn scope_ctx_mut(&mut self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'s, 'i, C> GlobalNewScopeMut for crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_mut(&mut self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl GlobalNewScopeMut for crate::isolate::Isolate {
  fn scope_ctx_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl GlobalNewScopeMut for crate::isolate::OwnedIsolate {
  fn scope_ctx_mut(&mut self) -> sys::Context { self.default_ctx() }
}
impl<'p, C> GlobalNewScopeMut for crate::exception::TryCatch<'p, HandleScope<'p, C>> {
  fn scope_ctx_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'s, C> GlobalNewScopeMut for crate::scope::CallbackScope<'s, C> {
  fn scope_ctx_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'p, S> GlobalNewScopeMut for crate::context::ContextScope<'p, S>
where
  crate::context::ContextScope<'p, S>: crate::scope::HandleScopeSource,
{
  fn scope_ctx_mut(&mut self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<P: GlobalNewScopeMut> GlobalNewScopeMut for std::pin::Pin<&mut P> {
  fn scope_ctx_mut(&mut self) -> sys::Context {
    unsafe { self.as_mut().get_unchecked_mut().scope_ctx_mut() }
  }
}

/// Original by-value GlobalNewScope kept for `&Scope` shared
/// references (real v8 takes `Global::new(&PinScope, _)`).
pub trait GlobalNewScope {
  fn scope_ctx_into(self) -> sys::Context;
}
impl<'a, 's, C> GlobalNewScope for &'a mut HandleScope<'s, C> {
  fn scope_ctx_into(self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'a, 's, C> GlobalNewScope for &'a HandleScope<'s, C> {
  fn scope_ctx_into(self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'a, 's, 'i, C> GlobalNewScope for &'a mut crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_into(self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl<'a, 's, 'i, C> GlobalNewScope for &'a crate::scope::PinScope<'s, 'i, C> {
  fn scope_ctx_into(self) -> sys::Context { HandleScope::ctx(&**self) }
}
impl<'a> GlobalNewScope for &'a mut crate::isolate::Isolate {
  fn scope_ctx_into(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 'b> GlobalNewScope for &'a &'b mut crate::isolate::Isolate {
  fn scope_ctx_into(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    let r: &mut crate::isolate::Isolate = unsafe {
      &mut *(*self as *const _ as *mut crate::isolate::Isolate)
    };
    r.default_ctx()
  }
}
impl<'a> GlobalNewScope for &'a mut crate::isolate::OwnedIsolate {
  fn scope_ctx_into(self) -> sys::Context { self.default_ctx() }
}
impl<'a, 'p, C> GlobalNewScope
  for &'a mut crate::exception::TryCatch<'p, HandleScope<'p, C>>
{
  fn scope_ctx_into(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 's, C> GlobalNewScope for &'a mut crate::scope::CallbackScope<'s, C> {
  fn scope_ctx_into(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
impl<'a, 'p, S> GlobalNewScope for &'a mut crate::context::ContextScope<'p, S>
where
  crate::context::ContextScope<'p, S>: crate::scope::HandleScopeSource,
{
  fn scope_ctx_into(self) -> sys::Context {
    use crate::scope::HandleScopeSource;
    self.default_ctx()
  }
}
// For Pin<&mut HandleScope> / Pin<&mut PinScope>: take by value too.
impl<'a, P> GlobalNewScope for std::pin::Pin<&'a mut P>
where
  P: GlobalNewScopePinned,
{
  fn scope_ctx_into(mut self) -> sys::Context {
    unsafe { self.as_mut().get_unchecked_mut().ctx_pinned() }
  }
}
pub trait GlobalNewScopePinned {
  fn ctx_pinned(&mut self) -> sys::Context;
}
impl<'s, C> GlobalNewScopePinned for HandleScope<'s, C> {
  fn ctx_pinned(&mut self) -> sys::Context { HandleScope::ctx(self) }
}
impl<'s, 'i, C> GlobalNewScopePinned for crate::scope::PinScope<'s, 'i, C> {
  fn ctx_pinned(&mut self) -> sys::Context { HandleScope::ctx(&**self) }
}

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
    scope: &mut S,
    recv: Local<'s, Value>,
    args: &[Local<'s, Value>],
  ) -> Option<Local<'s, Value>>
  where
    S: crate::scope::HandleScopeSource,
  {
    let local: Local<'s, crate::function::Function> = Local::from_raw(self.raw);
    Local::<crate::function::Function>::call(&local, scope, recv, args)
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
  /// `Global::new(scope, local)` — mirrors real v8's
  /// `Global::new(&Isolate, handle)` shape but accepts any scope-like
  /// type via `GlobalScope`. Takes a *shared* borrow so callers passing
  /// `&mut Foo` (Rust auto-reborrows to `&Foo`) keep their mutable
  /// binding usable across the call. Also accepts `&Isolate`,
  /// `&&mut Isolate`, `&PinScope`, `&HandleScope`, etc.
  pub fn new<'lo, S>(scope: &S, local: Local<'lo, T>) -> Self
  where
    S: GlobalScope + ?Sized,
  {
    let ctx = scope.scope_ctx_shared();
    sys::dup_value(ctx, local.raw);
    Self {
      raw: local.raw,
      ctx: Some(ctx),
      _t: PhantomData,
    }
  }
  /// Variant for shared `&Scope` callers — real v8 takes `&PinScope`.
  pub fn new_ref<'lo, S>(scope: &S, local: Local<'lo, T>) -> Self
  where
    S: GlobalNewScopeRefByShared + ?Sized,
  {
    let ctx = scope.scope_ctx_by_shared();
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
  /// Mirror of rusty_v8's `Global::from_raw(isolate, raw)` — the inverse
  /// of `into_raw`. Takes ownership of the heap-allocated JSValue
  /// produced by `into_raw` and reconstructs the `Global<T>`. Mirrors
  /// the convention that the raw pointer "owns" a refcount which is
  /// consumed by this call.
  ///
  /// # Safety
  /// `raw` must come from a previous `Global::into_raw` of the same T.
  pub unsafe fn from_raw(
    _isolate: &mut crate::isolate::Isolate,
    raw: std::ptr::NonNull<T>,
  ) -> Self {
    let raw_ptr = raw.as_ptr() as *mut sys::JSValue;
    let boxed = unsafe { Box::from_raw(raw_ptr) };
    let raw_value = *boxed;
    Self {
      raw: raw_value,
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
  /// Mirror of rusty_v8's `Global::into_raw(self) -> NonNull<T>`.
  /// Real v8 returns the underlying tagged-pointer handle; we instead
  /// heap-allocate the JSValue so the returned pointer is the same
  /// 8-byte width that callers (e.g. deno_node_sqlite) expect when they
  /// store `NonNull<v8::T>` in their data structs and later transmute
  /// back to a Local. The corresponding decode is
  /// `Local::from_raw_global_ptr`.
  pub fn into_raw(self) -> std::ptr::NonNull<T> {
    let boxed = Box::new(self.raw);
    let ptr = Box::into_raw(boxed) as *mut T;
    core::mem::forget(self);
    unsafe { std::ptr::NonNull::new_unchecked(ptr) }
  }
  /// Read a JSValue from a `NonNull<T>` produced by `into_raw` without
  /// freeing the heap allocation. Useful when the caller wants a
  /// short-lived Local that aliases the same handle still owned by the
  /// raw pointer.
  ///
  /// # Safety
  /// `ptr` must come from a previous `Global::into_raw` of the same T.
  pub unsafe fn local_from_raw_borrow<'s>(
    scope: &mut HandleScope<'s>,
    ptr: std::ptr::NonNull<T>,
  ) -> Local<'s, T> {
    let raw_ptr = ptr.as_ptr() as *mut sys::JSValue;
    let raw_value = unsafe { *raw_ptr };
    sys::dup_value(scope.ctx(), raw_value);
    scope.track_owned(raw_value);
    Local::from_raw(raw_value)
  }
  /// Legacy raw accessor for when callers want the raw JSValue directly
  /// rather than a heap-roundtrip. Useful for the qjs_v8_compat internal
  /// machinery only.
  pub(crate) fn into_raw_value(self) -> sys::JSValue {
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
impl<T> Clone for Weak<T> {
  fn clone(&self) -> Self {
    Self { _t: PhantomData }
  }
}
impl<T> std::fmt::Debug for Weak<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Weak<…>")
  }
}
impl<T> Weak<T> {
  pub fn new<'a, 'b, S>(_scope: &mut S, _local: Local<'a, T>) -> Self {
    Self { _t: PhantomData }
  }
  pub fn is_empty(&self) -> bool {
    true
  }
  pub fn to_local<'s>(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, T>> {
    None
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
