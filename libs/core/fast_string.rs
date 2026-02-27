// Copyright 2018-2025 the Deno authors. MIT license.

use serde::Deserializer;
use serde::Serializer;
use std::borrow::Borrow;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use url::Url;
use v8::NewStringType;

use crate::ToV8;

static EMPTY_STRING: v8::OneByteConst =
  v8::String::create_external_onebyte_const("".as_bytes());

/// A static string that is compile-time checked to be ASCII and is stored in the
/// most efficient possible way to create V8 strings.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct FastStaticString {
  s: &'static v8::OneByteConst,
}

impl FastStaticString {
  pub const fn new(s: &'static v8::OneByteConst) -> Self {
    FastStaticString { s }
  }

  pub fn as_str(&self) -> &'static str {
    self.s.as_ref()
  }

  pub fn as_bytes(&self) -> &'static [u8] {
    self.s.as_ref()
  }

  #[doc(hidden)]
  pub const fn create_external_onebyte_const(
    s: &'static [u8],
  ) -> v8::OneByteConst {
    v8::String::create_external_onebyte_const(s)
  }

  pub fn v8_string<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::String>, FastStringV8AllocationError> {
    FastString::from(*self).v8_string(scope)
  }

  pub const fn into_v8_const_ptr(&self) -> *const v8::OneByteConst {
    self.s as _
  }
}

impl From<&'static v8::OneByteConst> for FastStaticString {
  fn from(s: &'static v8::OneByteConst) -> Self {
    Self::new(s)
  }
}

impl From<FastStaticString> for *const v8::OneByteConst {
  fn from(val: FastStaticString) -> Self {
    val.into_v8_const_ptr()
  }
}

impl Hash for FastStaticString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_str().hash(state)
  }
}

impl AsRef<str> for FastStaticString {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl Deref for FastStaticString {
  type Target = str;
  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl Borrow<str> for FastStaticString {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}

impl Debug for FastStaticString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    Debug::fmt(self.as_str(), f)
  }
}

impl Default for FastStaticString {
  fn default() -> Self {
    FastStaticString { s: &EMPTY_STRING }
  }
}

impl PartialEq for FastStaticString {
  fn eq(&self, other: &Self) -> bool {
    self.as_bytes() == other.as_bytes()
  }
}

impl Eq for FastStaticString {}

impl Display for FastStaticString {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Debug, deno_error::JsError)]
#[class(type)]
pub struct FastStringV8AllocationError;

impl std::error::Error for FastStringV8AllocationError {}
impl std::fmt::Display for FastStringV8AllocationError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      f,
      "failed to allocate string; buffer exceeds maximum length"
    )
  }
}

/// Module names and code can be sourced from strings or bytes that are either owned or borrowed. This enumeration allows us
/// to perform a minimal amount of cloning and format-shifting of the underlying data.
///
/// Note that any [`FastString`] created using [`ascii_str!`] must contain only ASCII characters. Other [`FastString`] types
/// may be UTF-8, though this will incur a small performance penalty. It is recommended that large, static strings always
/// use [`ascii_str!`].
///
/// Examples of ways to construct a [`FastString`]:
///
/// ```rust
/// # use deno_core::{ascii_str, FastString};
///
/// let code: FastString = ascii_str!("a string").into();
/// let code: FastString = format!("a string").into();
/// ```
pub struct FastString {
  inner: FastStringInner,
}

enum FastStringInner {
  /// Created from static data.
  Static(&'static str),

  /// Created from static ascii, known to contain only ASCII chars.
  StaticAscii(&'static str),

  /// Created from static data, known to contain only ASCII chars.
  StaticConst(FastStaticString),

  /// An owned chunk of data. Note that we use `Box` rather than `Vec` to avoid the
  /// storage overhead.
  Owned(Box<str>),

  // Scripts loaded from the `deno_graph` infrastructure.
  Arc(Arc<str>),
}

impl FastString {
  /// Create a [`FastString`] from a static string. The string may contain
  /// non-ASCII characters, and if so, will take the slower path when used
  /// in v8.
  pub const fn from_static(s: &'static str) -> Self {
    if s.is_ascii() {
      Self {
        inner: FastStringInner::StaticAscii(s),
      }
    } else {
      Self {
        inner: FastStringInner::Static(s),
      }
    }
  }

  /// Create a [`FastString`] from a static string that is known to contain
  /// only ASCII characters.
  ///
  /// Note: This function is deliberately not `const fn`. Use `from_static`
  /// in const contexts.
  ///
  /// # Safety
  ///
  /// It is unsafe to specify a non-ASCII string here because this will be
  /// referenced in an external one byte static string in v8, which requires
  /// the data be Latin-1 or ASCII.
  ///
  /// This should only be used in scenarios where you know a string is ASCII
  /// and you want to avoid the performance overhead of checking if a string
  /// is ASCII that `from_static` does.
  pub unsafe fn from_ascii_static_unchecked(s: &'static str) -> Self {
    debug_assert!(
      s.is_ascii(),
      "use `from_non_ascii_static_unsafe` for non-ASCII strings",
    );
    Self {
      inner: FastStringInner::StaticAscii(s),
    }
  }

  /// Create a [`FastString`] from a static string that may contain non-ASCII
  /// characters.
  ///
  /// This should only be used in scenarios where you know a string is not ASCII
  /// and you want to avoid the performance overhead of checking if the string
  /// is ASCII that `from_static` does.
  ///
  /// Note: This function is deliberately not `const fn`. Use `from_static`
  /// in const contexts. This function is not unsafe because using this with
  /// an ascii string will just not be as optimal for performance.
  pub fn from_non_ascii_static(s: &'static str) -> Self {
    Self {
      inner: FastStringInner::Static(s),
    }
  }

  /// Returns a static string from this `FastString`, if available.
  pub fn as_static_str(&self) -> Option<&'static str> {
    match self.inner {
      FastStringInner::Static(s) => Some(s),
      FastStringInner::StaticAscii(s) => Some(s),
      FastStringInner::StaticConst(s) => Some(s.as_str()),
      _ => None,
    }
  }

  /// Creates a cheap copy of this [`FastString`], potentially transmuting it
  /// to a faster form. Note that this is not a clone operation as it consumes
  /// the old [`FastString`].
  pub fn into_cheap_copy(self) -> (Self, Self) {
    match self.inner {
      FastStringInner::Owned(s) => {
        let s: Arc<str> = s.into();
        (
          Self {
            inner: FastStringInner::Arc(s.clone()),
          },
          Self {
            inner: FastStringInner::Arc(s),
          },
        )
      }
      _ => (self.try_clone().unwrap(), self),
    }
  }

  /// If this [`FastString`] is cheaply cloneable, returns a clone.
  pub fn try_clone(&self) -> Option<Self> {
    match &self.inner {
      FastStringInner::Static(s) => Some(Self {
        inner: FastStringInner::Static(s),
      }),
      FastStringInner::StaticAscii(s) => Some(Self {
        inner: FastStringInner::StaticAscii(s),
      }),
      FastStringInner::StaticConst(s) => Some(Self {
        inner: FastStringInner::StaticConst(*s),
      }),
      FastStringInner::Arc(s) => Some(Self {
        inner: FastStringInner::Arc(s.clone()),
      }),
      FastStringInner::Owned(_s) => None,
    }
  }

  #[inline(always)]
  pub fn as_bytes(&self) -> &[u8] {
    self.as_str().as_bytes()
  }

  #[inline(always)]
  pub fn as_str(&self) -> &str {
    match &self.inner {
      // TODO(mmastrac): When we get a const deref, as_str can be const
      FastStringInner::Arc(s) => s,
      FastStringInner::Owned(s) => s,
      FastStringInner::Static(s) => s,
      FastStringInner::StaticAscii(s) => s,
      FastStringInner::StaticConst(s) => s.as_str(),
    }
  }

  /// Create a v8 string from this [`FastString`]. If the string is static and contains only ASCII characters,
  /// an external one-byte static is created.
  pub fn v8_string<'a, 'i>(
    &self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> Result<v8::Local<'a, v8::String>, FastStringV8AllocationError> {
    match self.inner {
      FastStringInner::StaticAscii(s) => {
        v8::String::new_external_onebyte_static(scope, s.as_bytes())
          .ok_or(FastStringV8AllocationError)
      }
      FastStringInner::StaticConst(s) => {
        v8::String::new_from_onebyte_const(scope, s.s)
          .ok_or(FastStringV8AllocationError)
      }
      _ => {
        v8::String::new_from_utf8(scope, self.as_bytes(), NewStringType::Normal)
          .ok_or(FastStringV8AllocationError)
      }
    }
  }

  /// Truncates a [`FastString`] value, possibly re-allocating or memcpy'ing. May be slow.
  pub fn truncate(&mut self, index: usize) {
    match &mut self.inner {
      FastStringInner::Static(b) => {
        self.inner = FastStringInner::Static(&b[..index])
      }
      FastStringInner::StaticAscii(b) => {
        self.inner = FastStringInner::StaticAscii(&b[..index])
      }
      FastStringInner::StaticConst(b) => {
        self.inner = FastStringInner::StaticAscii(&b.as_str()[..index])
      }
      // TODO(mmastrac): this could be more efficient
      FastStringInner::Owned(b) => {
        self.inner = FastStringInner::Owned(b[..index].to_owned().into())
      }
      // We can't do much if we have an Arc<str>, so we'll just take ownership of the truncated version
      FastStringInner::Arc(s) => {
        self.inner = FastStringInner::Arc(s[..index].to_owned().into())
      }
    }
  }
}

impl Hash for FastString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_str().hash(state)
  }
}

impl AsRef<str> for FastString {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl AsRef<[u8]> for FastString {
  fn as_ref(&self) -> &[u8] {
    self.as_str().as_ref()
  }
}

impl AsRef<OsStr> for FastString {
  fn as_ref(&self) -> &OsStr {
    self.as_str().as_ref()
  }
}

impl Deref for FastString {
  type Target = str;
  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl Borrow<str> for FastString {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}

impl Debug for FastString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    Debug::fmt(self.as_str(), f)
  }
}

impl Default for FastString {
  fn default() -> Self {
    Self {
      inner: FastStringInner::StaticConst(FastStaticString::default()),
    }
  }
}

impl PartialEq for FastString {
  fn eq(&self, other: &Self) -> bool {
    self.as_bytes() == other.as_bytes()
  }
}

impl Eq for FastString {}

impl Display for FastString {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

/// [`FastString`] can be made cheaply from [`Url`] as we know it's owned and don't need to do an
/// ASCII check.
impl From<FastStaticString> for FastString {
  fn from(value: FastStaticString) -> Self {
    Self {
      inner: FastStringInner::StaticConst(value),
    }
  }
}

/// [`FastString`] can be made cheaply from [`Url`] as we know it's owned and don't need to do an
/// ASCII check.
impl From<Url> for FastString {
  fn from(value: Url) -> Self {
    let s: String = value.into();
    s.into()
  }
}

/// [`FastString`] can be made cheaply from [`String`] as we know it's owned and don't need to do an
/// ASCII check.
impl From<String> for FastString {
  fn from(value: String) -> Self {
    Self {
      inner: FastStringInner::Owned(value.into_boxed_str()),
    }
  }
}

/// [`FastString`] can be made cheaply from [`Arc<str>`] as we know it's shared and don't need to do an
/// ASCII check.
impl From<Arc<str>> for FastString {
  fn from(value: Arc<str>) -> Self {
    Self {
      inner: FastStringInner::Arc(value),
    }
  }
}

impl From<FastString> for Arc<str> {
  fn from(value: FastString) -> Self {
    use FastStringInner::*;
    match value.inner {
      Static(text) | StaticAscii(text) => text.into(),
      StaticConst(text) => text.as_ref().into(),
      Owned(text) => text.into(),
      Arc(text) => text,
    }
  }
}

impl<'s> ToV8<'s> for FastString {
  type Error = FastStringV8AllocationError;

  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(self.v8_string(scope)?.into())
  }
}

impl serde::Serialize for FastString {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.as_str())
  }
}

type DeserializeProxy<'de> = &'de str;

impl<'de> serde::Deserialize<'de> for FastString {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    DeserializeProxy::<'de>::deserialize(deserializer)
      .map(|v| v.to_owned().into())
  }
}

/// Include a fast string in the binary. This string is asserted at compile-time to be 7-bit ASCII for optimal
/// v8 performance.
///
/// This macro creates a [`FastStaticString`] that may be converted to a [`FastString`] via [`Into::into`].
#[macro_export]
macro_rules! ascii_str_include {
  ($file:expr_2021) => {{
    const STR: $crate::v8::OneByteConst =
      $crate::FastStaticString::create_external_onebyte_const(
        ::std::include_str!($file).as_bytes(),
      );
    let s: &'static $crate::v8::OneByteConst = &STR;
    $crate::FastStaticString::new(s)
  }};
}

/// Include a fast string in the binary from a string literal. This string is asserted at compile-time to be
/// 7-bit ASCII for optimal v8 performance.
///
/// This macro creates a [`FastStaticString`] that may be converted to a [`FastString`] via [`Into::into`].
#[macro_export]
macro_rules! ascii_str {
  ($str:expr_2021) => {{
    const C: $crate::v8::OneByteConst =
      $crate::FastStaticString::create_external_onebyte_const($str.as_bytes());
    $crate::FastStaticString::new(&C)
  }};
}

/// Used to generate the fast, const versions of op names. Internal only.
#[macro_export]
#[doc(hidden)]
macro_rules! __op_name_fast {
  ($op:ident) => {{
    const LITERAL: &'static [u8] = stringify!($op).as_bytes();
    const STR: $crate::v8::OneByteConst =
      $crate::FastStaticString::create_external_onebyte_const(LITERAL);
    let s: &'static $crate::v8::OneByteConst = &STR;
    (stringify!($op), $crate::FastStaticString::new(s))
  }};
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn string_eq() {
    let s: FastString = ascii_str!("Testing").into();
    assert_eq!("Testing", s.as_str());
    let s2 = FastString::from_static("Testing");
    assert_eq!(s, s2);
    let (s1, s2) = s.into_cheap_copy();
    assert_eq!("Testing", s1.as_str());
    assert_eq!("Testing", s2.as_str());

    let s = FastString::from("Testing".to_owned());
    assert_eq!("Testing", s.as_str());
    let (s1, s2) = s.into_cheap_copy();
    assert_eq!("Testing", s1.as_str());
    assert_eq!("Testing", s2.as_str());
  }

  #[test]
  fn truncate() {
    let mut s = "123456".to_owned();
    s.truncate(3);

    let mut code: FastString = ascii_str!("123456").into();
    code.truncate(3);
    assert_eq!(s, code.as_str());

    let mut code: FastString = "123456".to_owned().into();
    code.truncate(3);
    assert_eq!(s, code.as_str());

    let arc_str: Arc<str> = "123456".into();
    let mut code: FastString = arc_str.into();
    code.truncate(3);
    assert_eq!(s, code.as_str());
  }

  #[test]
  fn test_large_include() {
    // This test would require an excessively large file in the repo, so we just run this manually
    // ascii_str_include!("runtime/tests/large_string.txt");
    // ascii_str_include!(concat!("runtime", "/tests/", "large_string.txt"));
  }

  /// Ensure that all of our macros compile properly in a static context.
  #[test]
  fn test_const() {
    const _: (&str, FastStaticString) = __op_name_fast!(op_name);
    const _: FastStaticString = ascii_str!("hmm");
    const _: FastStaticString = ascii_str!(concat!("hmm", "hmmmmm"));
    const _: FastStaticString = ascii_str_include!("Cargo.toml");
    const _: FastStaticString = ascii_str_include!(concat!("./", "Cargo.toml"));
  }
}
