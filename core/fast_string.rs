// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Borrow;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use url::Url;
use v8::NewStringType;

/// Module names and code can be sourced from strings or bytes that are either owned or borrowed. This enumeration allows us
/// to perform a minimal amount of cloning and format-shifting of the underlying data.
///
/// Note that any [`FastString`] created from a `'static` byte array or string must contain ASCII characters.
///
/// Examples of ways to construct a [`FastString`]:
///
/// ```rust
/// # use deno_core::{ascii_str, FastString};
///
/// let code: FastString = ascii_str!("a string");
/// let code: FastString = format!("a string").into();
/// ```
pub enum FastString {
  /// Created from static data.
  Static(&'static str),

  /// Created from static data, known to contain only ASCII chars.
  StaticAscii(&'static str),

  /// An owned chunk of data. Note that we use `Box` rather than `Vec` to avoid the
  /// storage overhead.
  Owned(Box<str>),

  // Scripts loaded from the `deno_graph` infrastructure.
  Arc(Arc<str>),
}

impl FastString {
  /// Compile-time function to determine if a string is ASCII. Note that UTF-8 chars
  /// longer than one byte have the high-bit set and thus, are not ASCII.
  const fn is_ascii(s: &'static [u8]) -> bool {
    let mut i = 0;
    while i < s.len() {
      if !s[i].is_ascii() {
        return false;
      }
      i += 1;
    }
    true
  }

  /// Create a [`FastString`] from a static string. The string may contain non-ASCII characters, and if
  /// so, will take the slower path when used in v8.
  pub const fn from_static(s: &'static str) -> Self {
    if Self::is_ascii(s.as_bytes()) {
      Self::StaticAscii(s)
    } else {
      Self::Static(s)
    }
  }

  /// Create a [`FastString`] from a static string. If the string contains non-ASCII characters, the compiler
  /// will abort.
  pub const fn ensure_static_ascii(s: &'static str) -> Self {
    if Self::is_ascii(s.as_bytes()) {
      Self::StaticAscii(s)
    } else {
      panic!("This string contained non-ASCII characters and cannot be created with ensure_static_ascii")
    }
  }

  /// Creates a cheap copy of this [`FastString`], potentially transmuting it to a faster form. Note that this
  /// is not a clone operation as it consumes the old [`FastString`].
  pub fn into_cheap_copy(self) -> (Self, Self) {
    match self {
      Self::Static(s) => (Self::Static(s), Self::Static(s)),
      Self::StaticAscii(s) => (Self::StaticAscii(s), Self::StaticAscii(s)),
      Self::Arc(s) => (Self::Arc(s.clone()), Self::Arc(s)),
      Self::Owned(s) => {
        let s: Arc<str> = s.into();
        (Self::Arc(s.clone()), Self::Arc(s))
      }
    }
  }

  pub const fn try_static_ascii(&self) -> Option<&'static [u8]> {
    match self {
      Self::StaticAscii(s) => Some(s.as_bytes()),
      _ => None,
    }
  }

  pub fn as_bytes(&self) -> &[u8] {
    // TODO(mmastrac): This can be const eventually (waiting for Arc const deref)
    match self {
      Self::Arc(s) => s.as_bytes(),
      Self::Owned(s) => s.as_bytes(),
      Self::Static(s) => s.as_bytes(),
      Self::StaticAscii(s) => s.as_bytes(),
    }
  }

  pub fn as_str(&self) -> &str {
    // TODO(mmastrac): This can be const eventually (waiting for Arc const deref)
    match self {
      Self::Arc(s) => s,
      Self::Owned(s) => s,
      Self::Static(s) => s,
      Self::StaticAscii(s) => s,
    }
  }

  /// Create a v8 string from this [`FastString`]. If the string is static and contains only ASCII characters,
  /// an external one-byte static is created.
  pub fn v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> v8::Local<'a, v8::String> {
    match self.try_static_ascii() {
      Some(s) => v8::String::new_external_onebyte_static(scope, s).unwrap(),
      None => {
        v8::String::new_from_utf8(scope, self.as_bytes(), NewStringType::Normal)
          .unwrap()
      }
    }
  }

  /// Truncates a [`FastString`] value, possibly re-allocating or memcpy'ing. May be slow.
  pub fn truncate(&mut self, index: usize) {
    match self {
      Self::Static(b) => *self = Self::Static(&b[..index]),
      Self::StaticAscii(b) => *self = Self::StaticAscii(&b[..index]),
      Self::Owned(b) => *self = Self::Owned(b[..index].to_owned().into()),
      // We can't do much if we have an Arc<str>, so we'll just take ownership of the truncated version
      Self::Arc(s) => *self = s[..index].to_owned().into(),
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
    Self::StaticAscii("")
  }
}

impl PartialEq for FastString {
  fn eq(&self, other: &Self) -> bool {
    self.as_bytes() == other.as_bytes()
  }
}

impl Eq for FastString {}

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
    FastString::Owned(value.into_boxed_str())
  }
}

/// [`FastString`] can be made cheaply from [`Arc<str>`] as we know it's shared and don't need to do an
/// ASCII check.
impl From<Arc<str>> for FastString {
  fn from(value: Arc<str>) -> Self {
    FastString::Arc(value)
  }
}

/// Include a fast string in the binary. This string is asserted at compile-time to be 7-bit ASCII for optimal
/// v8 performance.
#[macro_export]
macro_rules! include_ascii_string {
  ($file:literal) => {
    $crate::FastString::ensure_static_ascii(include_str!($file))
  };
}

/// Include a fast string in the binary from a string literal. This string is asserted at compile-time to be
/// 7-bit ASCII for optimal v8 performance.
#[macro_export]
macro_rules! ascii_str {
  ($str:literal) => {
    $crate::FastString::ensure_static_ascii($str)
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn truncate() {
    let mut s = "123456".to_owned();
    s.truncate(3);

    let mut code: FastString = FastString::from_static("123456");
    code.truncate(3);
    assert_eq!(s, code.as_ref());

    let mut code: FastString = "123456".to_owned().into();
    code.truncate(3);
    assert_eq!(s, code.as_ref());

    let arc_str: Arc<str> = "123456".into();
    let mut code: FastString = arc_str.into();
    code.truncate(3);
    assert_eq!(s, code.as_ref());
  }
}
