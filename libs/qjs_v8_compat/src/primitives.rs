// Copyright 2018-2026 the Deno authors. MIT license.
//
// Primitive types: String, Integer, Number, Boolean, BigInt, Symbol.

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Primitive;

// Marker types
crate::value_type!(
  String,
  Integer,
  Number,
  Boolean,
  BigInt,
  Symbol,
  PrimitiveArray
);

// ----- String -----------------------------------------------------------

#[derive(Default)]
pub enum NewStringType {
  #[default]
  Normal,
  Internalized,
}

// rusty_v8 calls these as inherent associated functions on the `String`
// marker type (e.g. `v8::String::new(scope, s)`). Mirror that surface.
// The body lives here (not on Local<String>) because the generic
// `Local::<T>::new(scope, &Global<T>)` impl in value.rs would otherwise
// duplicate-define `Local<'s, String>::new`.
impl String {
  pub fn new<'s, S: crate::value::LocalNewScopeRef<'s>>(
    scope: &S,
    s: &str,
  ) -> Option<Local<'s, String>> {
    let scope = scope.as_mut_handle_scope_ref();
    let raw = sys::new_string(scope.ctx(), s);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
  pub fn new_from_utf8<'s, S: crate::value::LocalNewScopeRef<'s>>(
    scope: &S,
    bytes: &[u8],
    ty: NewStringType,
  ) -> Option<Local<'s, String>> {
    let scope = scope.as_mut_handle_scope_ref();
    let _ = ty;
    std::str::from_utf8(bytes)
      .ok()
      .and_then(|s| Self::new(scope, s))
  }
  pub fn new_from_one_byte<'s>(
    scope: &mut HandleScope<'s>,
    bytes: &[u8],
    _ty: NewStringType,
  ) -> Option<Local<'s, String>> {
    let s = std::str::from_utf8(bytes).ok()?;
    Self::new(scope, s)
  }
  pub fn new_external_onebyte_static<'s, S: crate::value::LocalNewScopeRef<'s>>(
    scope: &S,
    bytes: &'static [u8],
  ) -> Option<Local<'s, String>> {
    let s = std::str::from_utf8(bytes).ok()?;
    Self::new(scope, s)
  }
  /// Mirror of `v8::String::new_external_onebyte` — same shape but
  /// accepts a non-static buffer. We just clone since we can't safely
  /// retain an `&[u8]` without lifetime through into Local.
  pub fn new_external_onebyte<'s>(
    scope: &mut HandleScope<'s>,
    bytes: Box<[u8]>,
  ) -> Option<Local<'s, String>> {
    let s = std::str::from_utf8(&bytes).ok()?;
    Self::new(scope, s)
  }
  pub fn empty<'s>(scope: &mut HandleScope<'s>) -> Local<'s, String> {
    Self::new(scope, "").unwrap()
  }
  /// Mirror of `String::new_from_two_byte` — UTF-16 input.
  pub fn new_from_two_byte<'s>(
    scope: &mut HandleScope<'s>,
    units: &[u16],
    _ty: NewStringType,
  ) -> Option<Local<'s, String>> {
    let s = std::string::String::from_utf16_lossy(units);
    Self::new(scope, &s)
  }
  pub fn new_from_onebyte_const<'s, S: crate::value::LocalNewScopeRef<'s>>(
    scope: &S,
    bytes: &'static OneByteConst,
  ) -> Option<Local<'s, String>> {
    let s = std::str::from_utf8(bytes._data).ok()?;
    Self::new(scope, s)
  }
  pub const fn create_external_onebyte_const(
    bytes: &'static [u8],
  ) -> OneByteConst {
    OneByteConst { _data: bytes }
  }
  // Marker-level method shims — deno_core sometimes invokes through
  // `&v8::String` (the marker, not `Local<String>`). The marker is a
  // ZST so these can't introspect the underlying value.
  pub fn length(&self) -> usize {
    0
  }
  pub fn is_onebyte(&self) -> bool {
    true
  }
  pub fn contains_only_onebyte(&self) -> bool {
    true
  }
  pub fn write_one_byte_uninit_v2<'sc, S>(
    &self,
    _scope: &mut S,
    _start: usize,
    _buf: &mut [std::mem::MaybeUninit<u8>],
    _flags: crate::v8::WriteFlags,
  ) -> usize {
    0
  }
  pub fn write_utf8_into<'sc, S, B: WriteUtf8Buf>(
    &self,
    scope: &mut S,
    buf: &mut B,
  ) -> (usize, usize)
  where
    S: crate::scope::HandleScopeSource,
  {
    let s = sys::to_string_lossy(scope.default_ctx(), self.raw)
      .unwrap_or_default();
    buf.append_str(&s);
    (s.len(), s.len())
  }
  pub fn to_rust_cow_lossy<'sc, 'b, S>(
    &self,
    scope: &mut S,
    _buf: &'b mut [std::mem::MaybeUninit<u8>],
  ) -> std::borrow::Cow<'b, str>
  where
    S: crate::scope::HandleScopeSource,
  {
    let s = sys::to_string_lossy(scope.default_ctx(), self.raw)
      .unwrap_or_default();
    std::borrow::Cow::Owned(s)
  }
}

impl<'s> Local<'s, String> {
  /// Mirror of `Local<String>::to_string(scope) -> Option<Local<String>>`.
  /// On a String input it's a no-op identity.
  pub fn to_string<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, String>> {
    Some(Local::from_raw(self.raw))
  }
  pub fn length(&self) -> usize {
    0
  }
  pub fn to_rust_cow_lossy<'sc, 'b>(
    &self,
    scope: &mut HandleScope<'sc>,
    _buf: &'b mut [std::mem::MaybeUninit<u8>],
  ) -> std::borrow::Cow<'b, str> {
    let s = sys::to_string_lossy(scope.ctx(), self.raw).unwrap_or_default();
    std::borrow::Cow::Owned(s)
  }
  pub fn utf8_length<'sc>(&self, _scope: &mut HandleScope<'sc>) -> usize {
    0
  }
  pub fn to_rust_string_lossy(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> std::string::String {
    sys::to_string_lossy(scope.ctx(), self.raw).unwrap_or_default()
  }
  pub fn is_onebyte(&self) -> bool {
    true
  }
  pub fn write_one_byte_uninit_v2<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _start: usize,
    buf: &mut [std::mem::MaybeUninit<u8>],
    _flags: crate::v8::WriteFlags,
  ) -> usize {
    let s = sys::to_string_lossy(_scope.ctx(), self.raw).unwrap_or_default();
    let n = s.len().min(buf.len());
    for (i, b) in s.as_bytes()[..n].iter().enumerate() {
      buf[i].write(*b);
    }
    n
  }
  pub fn write_utf8_into<'sc, B: WriteUtf8Buf>(
    &self,
    _scope: &mut HandleScope<'sc>,
    buf: &mut B,
  ) -> (usize, usize) {
    let s = sys::to_string_lossy(_scope.ctx(), self.raw).unwrap_or_default();
    buf.append_str(&s);
    (s.len(), s.len())
  }
  /// `v8::String::write_utf8_v2` — same shape as write_utf8_into but
  /// writes into raw bytes and returns nwritten.
  pub fn write_utf8_v2<'sc>(
    &self,
    scope: &mut HandleScope<'sc>,
    buf: &mut [u8],
    _flags: crate::v8::WriteFlags,
    _is_nul_terminated: Option<&mut usize>,
  ) -> usize {
    let s = sys::to_string_lossy(scope.ctx(), self.raw).unwrap_or_default();
    let n = s.len().min(buf.len());
    buf[..n].copy_from_slice(&s.as_bytes()[..n]);
    n
  }
}

/// Trait abstracting over the buffer types deno_core hands to
/// `write_utf8_into`. rusty_v8 has overloads for both `&mut [MaybeUninit<u8>]`
/// and `&mut String`; this trait covers both.
pub trait WriteUtf8Buf {
  fn append_str(&mut self, s: &str);
}
impl WriteUtf8Buf for [std::mem::MaybeUninit<u8>] {
  fn append_str(&mut self, s: &str) {
    let n = s.len().min(self.len());
    for (i, b) in s.as_bytes()[..n].iter().enumerate() {
      self[i].write(*b);
    }
  }
}
impl WriteUtf8Buf for std::string::String {
  fn append_str(&mut self, s: &str) {
    // Real V8's write_utf8_into overwrites the buffer. deno_core
    // reuses a thread-local String across calls without clearing it,
    // so an append-only impl bleeds previous strings into the next
    // call. Clear first to match real V8 semantics.
    self.clear();
    self.push_str(s);
  }
}
impl<T: WriteUtf8Buf + ?Sized> WriteUtf8Buf for std::cell::RefMut<'_, T> {
  fn append_str(&mut self, s: &str) {
    (**self).append_str(s);
  }
}
impl WriteUtf8Buf for Vec<u8> {
  fn append_str(&mut self, s: &str) {
    self.extend_from_slice(s.as_bytes());
  }
}

/// `OneByteConst` is V8's name for a statically allocated Latin-1 string.
/// QuickJS doesn't expose embed-able static strings to its C API directly;
/// we wrap a `&'static str` and intern-on-first-use.
pub struct OneByteConst {
  pub _data: &'static [u8],
}

impl OneByteConst {
  pub fn as_str(&self) -> &str {
    std::str::from_utf8(self._data).unwrap_or("")
  }
}
impl AsRef<str> for OneByteConst {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}
impl AsRef<[u8]> for OneByteConst {
  fn as_ref(&self) -> &[u8] {
    self._data
  }
}

// ----- Integer / Number / Boolean / BigInt -----------------------------
//
// `new` constructors live as inherent methods on the marker types
// themselves (mirroring rusty_v8) so they don't conflict with the
// generic `Local::<T>::new(scope, &Global<T>)` impl in value.rs.

impl Integer {
  pub fn new<'s, S>(_scope: &S, v: i32) -> Local<'s, Integer> {
    Local::from_raw(sys::jsv_int32(v))
  }
  pub fn new_from_unsigned<'s, S>(
    _scope: &S,
    v: u32,
  ) -> Local<'s, Integer> {
    if v <= i32::MAX as u32 {
      Local::from_raw(sys::jsv_int32(v as i32))
    } else {
      Local::from_raw(sys::jsv_float64(v as f64))
    }
  }
}

impl<'s> Local<'s, Integer> {
  pub fn value(&self) -> i64 {
    match self.raw.tag {
      sys::JS_TAG_INT => unsafe { self.raw.u.int32 as i64 },
      sys::JS_TAG_FLOAT64 => unsafe { self.raw.u.float64 as i64 },
      _ => 0,
    }
  }
  pub fn int32_value<S>(&self, _scope: &mut S) -> Option<i32> {
    Some(self.value() as i32)
  }
}

// Integer the marker type also gets a value() — deno_core occasionally
// dispatches through `(&Integer).value()` (via auto-deref or pattern
// destructure) and expects a marker-level method.
impl Integer {
  pub fn value(&self) -> i64 {
    0
  }
}
impl Number {
  pub fn value_marker(&self) -> f64 {
    0.0
  }
}

impl Number {
  pub fn new<'s, S: crate::value::LocalNewScopeRef<'s>>(_scope: &S, v: f64) -> Local<'s, Number> {
    Local::from_raw(sys::jsv_float64(v))
  }
  /// Mirror of `v8::Number::value`. ZST markers can't store the value;
  /// this returns 0 — real callsites should go through Local::value().
  pub fn value(&self) -> f64 {
    0.0
  }
}

impl<'s> Local<'s, Number> {
  pub fn value(&self) -> f64 {
    match self.raw.tag {
      sys::JS_TAG_INT => unsafe { self.raw.u.int32 as f64 },
      sys::JS_TAG_FLOAT64 => unsafe { self.raw.u.float64 },
      _ => f64::NAN,
    }
  }
  pub fn integer_value<S>(&self, _scope: &mut S) -> Option<i64>
  where
    S: crate::scope::HandleScopeSource + ?Sized,
  {
    Some(self.value() as i64)
  }
  pub fn uint32_value<S>(&self, _scope: &mut S) -> Option<u32>
  where
    S: crate::scope::HandleScopeSource + ?Sized,
  {
    Some(self.value() as u32)
  }
  pub fn int32_value<S>(&self, _scope: &mut S) -> Option<i32>
  where
    S: crate::scope::HandleScopeSource + ?Sized,
  {
    Some(self.value() as i32)
  }
  pub fn number_value<S>(&self, _scope: &mut S) -> Option<f64>
  where
    S: crate::scope::HandleScopeSource + ?Sized,
  {
    Some(self.value())
  }
  /// `v8::Number::to_string(scope)` — coerce to v8::String.
  pub fn to_string<'sc>(
    &self,
    scope: &mut HandleScope<'sc>,
  ) -> Option<Local<'sc, String>> {
    let s = self.value().to_string();
    String::new(scope, &s)
  }
}

impl Boolean {
  pub fn new<'s, S: crate::value::LocalNewScopeRef<'s>>(_scope: &S, v: bool) -> Local<'s, Boolean> {
    Local::from_raw(sys::jsv_bool(v))
  }
}

impl<'s> Local<'s, Boolean> {
  pub fn is_true(&self) -> bool {
    sys::jsv_is_bool(&self.raw) && unsafe { self.raw.u.int32 != 0 }
  }
}

impl BigInt {
  /// Mirror of `BigInt::new_from_words` — construct from sign + words.
  /// Stub on QuickJS.
  pub fn new_from_words<'s>(
    scope: &mut HandleScope<'s>,
    _sign_bit: bool,
    _words: &[u64],
  ) -> Option<Local<'s, BigInt>> {
    Some(Self::new_from_i64(scope, 0))
  }
  pub fn new_from_u64<'s>(
    scope: &mut HandleScope<'s>,
    v: u64,
  ) -> Local<'s, BigInt> {
    Self::new_from_i64(scope, v as i64)
  }
  /// Mirror of `BigInt::u64_value` on the marker type. Stub.
  pub fn u64_value(&self) -> (u64, bool) {
    (0, false)
  }
  pub fn i64_value(&self) -> (i64, bool) {
    (0, false)
  }
  pub fn new_from_i64<'s>(
    scope: &mut HandleScope<'s>,
    _v: i64,
  ) -> Local<'s, BigInt> {
    // QJS-DIVERGE: a real implementation routes through JS_NewBigInt64. We
    // pretend with a tagged sentinel until JS_NewBigInt64 wiring is added
    // to sys.rs.
    let raw = sys::JSValue {
      u: sys::JSValueUnion { int32: 0 },
      tag: sys::JS_TAG_BIG_INT,
    };
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
}

impl Symbol {
  /// Mirror of `v8::Symbol::for_key(scope, key)`.
  pub fn for_key<'s>(
    scope: &mut HandleScope<'s>,
    _key: Local<'s, String>,
  ) -> Local<'s, Symbol> {
    Self::new_inner(scope)
  }
  /// Mirror of `v8::Symbol::new(scope, description)`.
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    _description: Option<Local<'_, String>>,
  ) -> Local<'s, Symbol> {
    Self::new_inner(scope)
  }
  fn new_inner<'s>(_scope: &mut HandleScope<'s>) -> Local<'s, Symbol> {
    // QJS-DIVERGE: real path is JS_NewSymbol(ctx, NULL, false). Mocked.
    // Do NOT track_owned — this is a fake JSValue with a NULL pointer
    // payload, and JS_FreeValue would dereference it on scope drop and
    // corrupt the runtime atom table.
    let raw = sys::JSValue {
      u: sys::JSValueUnion { int32: 0 },
      tag: sys::JS_TAG_SYMBOL,
    };
    Local::from_raw(raw)
  }
  pub fn for_<'s>(
    scope: &mut HandleScope<'s>,
    _desc: Local<'s, String>,
  ) -> Local<'s, Symbol> {
    Self::new_inner(scope)
  }
  pub fn get_iterator<'s, S>(_scope: &mut S) -> Local<'s, Symbol> {
    Local::from_raw(sys::JSValue {
      u: sys::JSValueUnion { int32: 0 },
      tag: sys::JS_TAG_SYMBOL,
    })
  }
  pub fn get_async_iterator<'s, S>(_scope: &mut S) -> Local<'s, Symbol> {
    Local::from_raw(sys::JSValue {
      u: sys::JSValueUnion { int32: 0 },
      tag: sys::JS_TAG_SYMBOL,
    })
  }
}

// ----- Primitive helpers -----------------------------------------------

impl Primitive {
  pub(crate) fn undefined<'s>(
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, Primitive> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub(crate) fn null<'s>(_scope: &mut HandleScope<'s>) -> Local<'s, Primitive> {
    Local::from_raw(sys::jsv_null())
  }
}

// `value_type!` macro local to this crate. Defined once here, exported via
// `pub(crate) use`.
#[macro_export]
#[doc(hidden)]
macro_rules! value_type {
  ($($name:ident),* $(,)?) => {
    $(
      // Each v8 type marker carries the JSValue raw (same first
      // field as `Local<T>`) so methods on `&v8::Foo` (via
      // `Local<Foo>::deref()` casting through us) can read tag/u
      // and answer queries like `is_string()` accurately. Without
      // this they were ZSTs and every predicate returned false,
      // which broke op2-emitted code that does
      // `arg0.is_string()` on a `&v8::Value` borrowed from
      // `Local<Value>::deref()`.
      #[derive(Copy, Clone)]
      #[repr(transparent)]
      pub struct $name {
        pub(crate) raw: $crate::sys::JSValue,
      }
    )*
  };
}

// PrimitiveArray methods
impl PrimitiveArray {
  pub fn new<'s>(
    _scope: &mut HandleScope<'s>,
    _length: i32,
  ) -> Local<'s, PrimitiveArray> {
    Local::from_raw(sys::jsv_undefined())
  }
}
impl<'s> Local<'s, PrimitiveArray> {
  pub fn length(&self) -> i32 {
    0
  }
  pub fn set(
    &self,
    _scope: &mut HandleScope<'s>,
    _index: i32,
    _value: Local<'s, Primitive>,
  ) {
  }
  pub fn get(
    &self,
    _scope: &mut HandleScope<'s>,
    _index: i32,
  ) -> Local<'s, Primitive> {
    Primitive::undefined(_scope)
  }
}
