// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken) Use f64::maximum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
pub(crate) fn maximum(a: f64, b: f64) -> f64 {
  if a > b {
    a
  } else if b > a {
    b
  } else if a == b {
    if a.is_sign_positive() && b.is_sign_negative() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
}

// TODO(petamoriken) Use f64::minimum instead https://github.com/rust-lang/rust/issues/91079
#[inline]
pub(crate) fn minimum(a: f64, b: f64) -> f64 {
  if a < b {
    a
  } else if b < a {
    b
  } else if a == b {
    if a.is_sign_negative() && b.is_sign_positive() {
      a
    } else {
      b
    }
  } else {
    // At least one input is NaN. Use `+` to perform NaN propagation and quieting.
    a + b
  }
}
