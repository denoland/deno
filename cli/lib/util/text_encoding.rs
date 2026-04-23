// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

pub fn is_valid_utf8(bytes: &[u8]) -> bool {
  matches!(String::from_utf8_lossy(bytes), Cow::Borrowed(_))
}

// todo(https://github.com/rust-lang/rust/issues/129436): remove once stabilized
#[inline(always)]
pub fn from_utf8_lossy_owned(bytes: Vec<u8>) -> String {
  match String::from_utf8_lossy(&bytes) {
    Cow::Owned(code) => code,
    // SAFETY: `String::from_utf8_lossy` guarantees that the result is valid
    // UTF-8 if `Cow::Borrowed` is returned.
    Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(bytes) },
  }
}

#[inline(always)]
pub fn from_utf8_lossy_cow(bytes: Cow<'_, [u8]>) -> Cow<'_, str> {
  match bytes {
    Cow::Borrowed(bytes) => String::from_utf8_lossy(bytes),
    Cow::Owned(bytes) => Cow::Owned(from_utf8_lossy_owned(bytes)),
  }
}
