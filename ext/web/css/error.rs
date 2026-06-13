// Copyright 2018-2026 the Deno authors. MIT license.

use cssparser::BasicParseErrorKind;
use cssparser::ParseErrorKind;

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CSSCustomError {
  #[error("unexpected numeric type")]
  UnexpectedNumericType,
  #[error(
    "contains relative <length> values that cannot be resolved at parse time"
  )]
  ContainsRelativeLengthValues,
  #[error("contains {0} calculations that cannot be resolved at parse time")]
  ContainPercentAndDimensionCalculations(&'static str),
  #[error("cannot add or subtract different numeric types")]
  NumericTypeMismatch,
  #[error("the dimension of the calculation result is incorrect")]
  InvalidDimension,
  #[error("contains invalid function: {0}")]
  InvalidFunction(String),
  #[error("invalid CSS color string")]
  InvalidColor,
}

pub type CSSParseError<'i> = cssparser::ParseError<'i, CSSCustomError>;

/// Convert a CSS parse error to a user-friendly string.
///
/// Avoids verbose `Token` debug output for `UnexpectedToken` errors.
pub fn css_parse_error_to_string(error: CSSParseError<'_>) -> String {
  match error.kind {
    ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(_)) => {
      "unexpected token".into()
    }
    _ => format!("{}", error),
  }
}
