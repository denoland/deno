// Copyright 2018-2025 the Deno authors. MIT license.

use chrono::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseIso8601DurationError {
  #[error("empty duration string")]
  Empty,

  #[error("duration must start with 'P'")]
  MissingP,

  #[error("expected designators after 'P'")]
  MissingDesignator,

  #[error("expected time component after 'T'")]
  MissingTimeAfterT,

  #[error("duplicate 'T' designator")]
  DuplicateT,

  #[error("expected number")]
  ExpectedNumber,

  #[error("expected digits after decimal point")]
  ExpectedFraction,

  #[error("missing unit designator")]
  MissingUnit,

  #[error("invalid integer number")]
  InvalidNumber,

  #[error("invalid fractional seconds")]
  InvalidFractionalSeconds,

  #[error("months not supported")]
  MonthsNotSupported,

  #[error("years not supported")]
  YearsNotSupported,

  #[error("weeks must be the only component (use PnW)")]
  WeeksMustBeAlone,

  #[error("fractional value is only allowed for seconds (S)")]
  FractionalNotAllowed,

  #[error("invalid unit designator")]
  InvalidUnit,

  #[error("duration overflow")]
  Overflow,
}

/// Parses ISO-8601 durations of the form:
///   PnW | PnDTnHnMnS | PnD | PTnH | PTnM | PTnS | combinations
/// Notes:
/// - Supports optional leading '-' for negative durations.
/// - Supports weeks (W), days (D), hours (H), minutes (M in the *time* part), seconds (S).
/// - DOES NOT support years or months (ambiguous length); will error if present.
pub fn parse_iso8601_duration(
  input: &str,
) -> Result<Duration, ParseIso8601DurationError> {
  if input.is_empty() {
    return Err(ParseIso8601DurationError::Empty);
  }

  // accept optional '+' or '-' sign
  let (s, neg) = match input.strip_prefix(['-', '+']) {
    Some(rest) if input.starts_with('-') => (rest, true),
    Some(rest) => (rest, false),
    None => (input, false),
  };

  let Some(s) = s.strip_prefix('P') else {
    return Err(ParseIso8601DurationError::MissingP);
  };
  if s.is_empty() {
    return Err(ParseIso8601DurationError::MissingDesignator);
  }

  // weeks-only short form
  if let Some(num) = s.strip_suffix(['W', 'w']) {
    let weeks: i64 = num
      .parse()
      .map_err(|_| ParseIso8601DurationError::InvalidNumber)?;
    let days = weeks
      .checked_mul(7)
      .ok_or(ParseIso8601DurationError::Overflow)?;
    let d = Duration::days(days);
    return Ok(if neg { -d } else { d });
  }

  let bytes = s.as_bytes();
  let mut i = 0usize;
  let mut in_time = false;
  let mut total = Duration::zero();

  while i < bytes.len() {
    if !in_time && bytes[i] == b'T' {
      in_time = true;
      i += 1;
      if i == bytes.len() {
        return Err(ParseIso8601DurationError::MissingTimeAfterT);
      }
      continue;
    } else if in_time && bytes[i] == b'T' {
      return Err(ParseIso8601DurationError::DuplicateT);
    }

    // parse integer part
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
      i += 1;
    }
    if start == i {
      return Err(ParseIso8601DurationError::ExpectedNumber);
    }

    // optional fractional part ONLY allowed for seconds
    let mut frac_start = i;
    let mut frac_len = 0usize;
    if i < bytes.len() && bytes[i] == b'.' {
      i += 1;
      frac_start = i;
      while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        frac_len += 1;
      }
      if frac_len == 0 {
        return Err(ParseIso8601DurationError::ExpectedFraction);
      }
    }

    if i >= bytes.len() {
      return Err(ParseIso8601DurationError::MissingUnit);
    }
    let mut unit = bytes[i] as char;
    unit.make_ascii_uppercase();
    i += 1;

    // integral value
    let int_val: i64 = s[start..start + (frac_start - start)]
      .parse()
      .map_err(|_| ParseIso8601DurationError::InvalidNumber)?;

    // add to total
    let add = match (in_time, unit, frac_len) {
      (false, 'D', 0) => Duration::days(int_val),
      (false, 'M', _) => {
        return Err(ParseIso8601DurationError::MonthsNotSupported);
      }
      (false, 'Y', _) => {
        return Err(ParseIso8601DurationError::YearsNotSupported);
      }
      (false, 'W', _) => {
        return Err(ParseIso8601DurationError::WeeksMustBeAlone);
      }

      (true, 'H', 0) => Duration::hours(int_val),
      (true, 'M', 0) => Duration::minutes(int_val),
      // Seconds may be fractional: PT1.5S
      (true, 'S', _) => {
        let mut d = Duration::seconds(int_val);
        if frac_len > 0 {
          let frac_str = &s[frac_start..frac_start + frac_len];
          let nanos = {
            let n = frac_str.chars().take(9).collect::<String>();
            let scale = 9 - n.len();
            let base: i64 = n.parse().map_err(|_| {
              ParseIso8601DurationError::InvalidFractionalSeconds
            })?;
            base * 10_i64.pow(scale as u32)
          };
          d = d + Duration::nanoseconds(nanos);
        }
        d
      }

      (true, _, _) => {
        return Err(ParseIso8601DurationError::FractionalNotAllowed);
      }
      _ => return Err(ParseIso8601DurationError::InvalidUnit),
    };

    total = total
      .checked_add(&add)
      .ok_or(ParseIso8601DurationError::Overflow)?;
  }

  Ok(if neg { -total } else { total })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ok_basic() {
    assert_eq!(
      parse_iso8601_duration("P3DT4H12M6S").unwrap(),
      Duration::days(3)
        + Duration::hours(4)
        + Duration::minutes(12)
        + Duration::seconds(6)
    );
    assert_eq!(
      parse_iso8601_duration("+PT90S").unwrap(),
      Duration::seconds(90)
    );
    assert_eq!(parse_iso8601_duration("P2W").unwrap(), Duration::days(14));
    assert_eq!(
      parse_iso8601_duration("PT1.5S").unwrap(),
      Duration::milliseconds(1500)
    );
    assert_eq!(
      parse_iso8601_duration("-PT5M").unwrap(),
      -Duration::minutes(5)
    );
  }

  #[test]
  fn errs() {
    assert!(parse_iso8601_duration("P1Y").is_err());
    assert!(parse_iso8601_duration("P1M").is_err());
    assert!(parse_iso8601_duration("PT").is_err());
    assert!(parse_iso8601_duration("PT1.2M").is_err()); // fractional minutes rejected
    assert!(parse_iso8601_duration("P1WT1H").is_err()); // W must be alone
  }
}
