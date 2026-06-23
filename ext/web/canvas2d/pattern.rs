// Copyright 2018-2026 the Deno authors. MIT license.

use vello::peniko;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PatternError {
  #[class("DOMExceptionSyntaxError")]
  #[error("The string did not match the expected pattern.")]
  Syntax,
}

/// Parsed repetition modes for Canvas 2D createPattern().
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatternRepetition {
  pub x_extend: peniko::Extend,
  pub y_extend: peniko::Extend,
}

/// Parses the repetition argument for createPattern().
///
/// `null` should be normalized to `""` by the caller before invoking this function.
pub fn parse_repetition(s: &str) -> Result<PatternRepetition, PatternError> {
  match s {
    "" | "repeat" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Repeat,
      y_extend: peniko::Extend::Repeat,
    }),
    "repeat-x" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Repeat,
      y_extend: peniko::Extend::Pad,
    }),
    "repeat-y" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Pad,
      y_extend: peniko::Extend::Repeat,
    }),
    "no-repeat" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Pad,
      y_extend: peniko::Extend::Pad,
    }),
    "null" | "undefined" => Err(PatternError::Syntax),
    _ => Err(PatternError::Syntax),
  }
}
