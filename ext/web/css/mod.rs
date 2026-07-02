// Copyright 2018-2026 the Deno authors. MIT license.

/// Extract a typed value from a [`NumericValue`](value::NumericValue) via one
/// of its `expect_*` methods, converting the `CSSCustomError` into a
/// `CSSParseError` on failure.
macro_rules! try_extract {
  ($expr:expr, $method:ident($($arg:expr),*), $input:expr) => {
    match $expr.$method($($arg),*) {
      Ok(v) => v,
      Err(e) => return Err($input.new_custom_error(e)),
    }
  };
  ($expr:expr, $method:ident($($arg:expr),*), $map:ident(), $input:expr) => {
    match $expr.$method($($arg),*) {
      Ok(v) => v.$map(),
      Err(e) => return Err($input.new_custom_error(e)),
    }
  };
}

pub mod color;
pub mod error;
pub mod filter;
pub mod font;
pub mod transform;
pub mod value;
