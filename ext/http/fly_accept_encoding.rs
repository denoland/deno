// Copyright 2018 Yoshua Wuyts. All rights reserved. MIT license.
// Copyright 2018-2025 the Deno authors. MIT license.
// Forked from https://github.com/superfly/accept-encoding/blob/1cded757ec7ff3916e5bfe7441db76cdc48170dc/
// Forked to support both http 0.3 and http 1.0 crates.

use itertools::Itertools;

/// A list enumerating the categories of errors in this crate.
///
/// This list is intended to grow over time and it is not recommended to
/// exhaustively match against it.
///
/// It is used with the [`Error`] struct.
///
/// [`Error`]: std.struct.Error.html
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
  /// Invalid header encoding.
  #[error("Invalid header encoding.")]
  InvalidEncoding,
  /// The encoding scheme is unknown.
  #[error("Unknown encoding scheme.")]
  UnknownEncoding,
}

/// Encodings to use.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Encoding {
  /// The Gzip encoding.
  Gzip,
  /// The Deflate encoding.
  Deflate,
  /// The Brotli encoding.
  Brotli,
  /// The Zstd encoding.
  Zstd,
  /// No encoding.
  Identity,
}

impl Encoding {
  /// Parses a given string into its corresponding encoding.
  fn parse(s: &str) -> Result<Option<Encoding>, EncodingError> {
    match s {
      "gzip" => Ok(Some(Encoding::Gzip)),
      "deflate" => Ok(Some(Encoding::Deflate)),
      "br" => Ok(Some(Encoding::Brotli)),
      "zstd" => Ok(Some(Encoding::Zstd)),
      "identity" => Ok(Some(Encoding::Identity)),
      "*" => Ok(None),
      _ => Err(EncodingError::UnknownEncoding),
    }
  }
}

/// Select the encoding with the largest qval or the first with qval ~= 1
pub fn preferred(
  encodings: impl Iterator<Item = Result<(Option<Encoding>, f32), EncodingError>>,
) -> Result<Option<Encoding>, EncodingError> {
  let mut preferred_encoding = None;
  let mut max_qval = 0.0;

  for r in encodings {
    let (encoding, qval) = r?;
    if (qval - 1.0f32).abs() < 0.01 {
      return Ok(encoding);
    } else if qval > max_qval {
      preferred_encoding = encoding;
      max_qval = qval;
    }
  }

  Ok(preferred_encoding)
}

/// Parse a set of HTTP headers into an iterator containing tuples of options containing encodings and their corresponding q-values.
///
/// Compatible with `http` crate for version 1.x.
pub fn encodings_iter_http_1(
  headers: &http::HeaderMap,
) -> impl Iterator<Item = Result<(Option<Encoding>, f32), EncodingError>> + '_ {
  let iter = headers
    .get_all(http::header::ACCEPT_ENCODING)
    .iter()
    .map(|hval| hval.to_str().map_err(|_| EncodingError::InvalidEncoding));
  encodings_iter_inner(iter)
}

/// Parse a set of HTTP headers into an iterator containing tuples of options containing encodings and their corresponding q-values.
fn encodings_iter_inner<'s>(
  headers: impl Iterator<Item = Result<&'s str, EncodingError>> + 's,
) -> impl Iterator<Item = Result<(Option<Encoding>, f32), EncodingError>> + 's {
  headers
    .map_ok(|s| s.split(',').map(str::trim))
    .flatten_ok()
    .filter_map_ok(|v| {
      let (e, q) = match v.split_once(";q=") {
        Some((e, q)) => (e, q),
        None => return Some(Ok((Encoding::parse(v).ok()?, 1.0f32))),
      };
      let encoding = Encoding::parse(e).ok()?; // ignore unknown encodings
      let qval = match q.parse() {
        Ok(f) if f > 1.0 => return Some(Err(EncodingError::InvalidEncoding)), // q-values over 1 are unacceptable,
        Ok(f) => f,
        Err(_) => return Some(Err(EncodingError::InvalidEncoding)),
      };
      Some(Ok((encoding, qval)))
    })
    .flatten()
}
