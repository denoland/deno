// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

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

/// Converts an `Arc<str>` to an `Arc<[u8]>`.
#[allow(dead_code)]
pub fn arc_str_to_bytes(arc_str: Arc<str>) -> Arc<[u8]> {
  let raw = Arc::into_raw(arc_str);
  // SAFETY: This is safe because they have the same memory layout.
  unsafe { Arc::from_raw(raw as *const [u8]) }
}

/// Converts an `Arc<u8>` to an `Arc<str>` if able.
#[allow(dead_code)]
pub fn arc_u8_to_arc_str(
  arc_u8: Arc<[u8]>,
) -> Result<Arc<str>, std::str::Utf8Error> {
  // Check that the string is valid UTF-8.
  std::str::from_utf8(&arc_u8)?;
  // SAFETY: the string is valid UTF-8, and the layout Arc<[u8]> is the same as
  // Arc<str>. This is proven by the From<Arc<str>> impl for Arc<[u8]> from the
  // standard library.
  Ok(unsafe {
    std::mem::transmute::<std::sync::Arc<[u8]>, std::sync::Arc<str>>(arc_u8)
  })
}
