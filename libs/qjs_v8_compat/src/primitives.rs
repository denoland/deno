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
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    s: &str,
  ) -> Option<Local<'s, String>> {
    let raw = sys::new_string(scope.ctx(), s);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
  pub fn new_from_utf8<'s>(
    scope: &mut HandleScope<'s>,
    bytes: &[u8],
    ty: NewStringType,
  ) -> Option<Local<'s, String>> {
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
  pub fn new_external_onebyte_static<'s>(
    scope: &mut HandleScope<'s>,
    bytes: &'static [u8],
  ) -> Option<Local<'s, String>> {
    let s = std::str::from_utf8(bytes).ok()?;
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
}

impl<'s> Local<'s, String> {
  pub fn length(&self) -> usize {
    // Returning byte length of the UTF-8 form is an approximation; V8 uses
    // UTF-16 code units. Refined later.
    0
  }
  pub fn utf8_length(&self, _scope: &mut HandleScope<'s>) -> usize {
    0
  }
  pub fn to_rust_string_lossy(
    &self,
    scope: &mut HandleScope<'s>,
  ) -> std::string::String {
    sys::to_string_lossy(scope.ctx(), self.raw).unwrap_or_default()
  }
}

/// `OneByteConst` is V8's name for a statically allocated Latin-1 string.
/// QuickJS doesn't expose embed-able static strings to its C API directly;
/// we wrap a `&'static str` and intern-on-first-use.
pub struct OneByteConst {
  pub data: &'static str,
}

// ----- Integer / Number / Boolean / BigInt -----------------------------
//
// `new` constructors live as inherent methods on the marker types
// themselves (mirroring rusty_v8) so they don't conflict with the
// generic `Local::<T>::new(scope, &Global<T>)` impl in value.rs.

impl Integer {
  pub fn new<'s>(_scope: &mut HandleScope<'s>, v: i32) -> Local<'s, Integer> {
    Local::from_raw(sys::jsv_int32(v))
  }
  pub fn new_from_unsigned<'s>(
    _scope: &mut HandleScope<'s>,
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
}

impl Number {
  pub fn new<'s>(_scope: &mut HandleScope<'s>, v: f64) -> Local<'s, Number> {
    Local::from_raw(sys::jsv_float64(v))
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
}

impl Boolean {
  pub fn new<'s>(_scope: &mut HandleScope<'s>, v: bool) -> Local<'s, Boolean> {
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
  pub fn new<'s>(scope: &mut HandleScope<'s>) -> Local<'s, Symbol> {
    // QJS-DIVERGE: real path is JS_NewSymbol(ctx, NULL, false). Mocked.
    let raw = sys::JSValue {
      u: sys::JSValueUnion { int32: 0 },
      tag: sys::JS_TAG_SYMBOL,
    };
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  pub fn for_<'s>(
    scope: &mut HandleScope<'s>,
    _desc: Local<'s, String>,
  ) -> Local<'s, Symbol> {
    Self::new(scope)
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
      #[derive(Copy, Clone)]
      pub struct $name { _private: () }
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
