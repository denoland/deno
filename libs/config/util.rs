// Copyright 2018-2025 the Deno authors. MIT license.

use chrono::DateTime;
use chrono::Duration;
use thiserror::Error;

use crate::deno_json::NewestDependencyDate;

pub fn is_skippable_io_error(e: &std::io::Error) -> bool {
  use std::io::ErrorKind::*;

  // skip over invalid filenames on windows
  const ERROR_INVALID_NAME: i32 = 123;
  if cfg!(windows) && e.raw_os_error() == Some(ERROR_INVALID_NAME) {
    return true;
  }

  match e.kind() {
    InvalidInput | PermissionDenied | NotFound => {
      // ok keep going
      true
    }
    _ => {
      const NOT_A_DIRECTORY: i32 = 20;
      cfg!(unix) && e.raw_os_error() == Some(NOT_A_DIRECTORY)
    }
  }
}

#[derive(Debug, Error)]
pub enum ParseDateOrDurationError {
  #[error("failed parsing integer to minutes")]
  InvalidMinutes(#[source] std::num::ParseIntError),

  #[error("expected minutes, RFC3339 datetime, or ISO-8601 duration")]
  InvalidDateTime(#[source] chrono::ParseError),

  #[error("expected minutes, RFC3339 datetime, or ISO-8601 duration")]
  InvalidDuration(#[source] ParseIso8601DurationError),
}

/// Parses a string that could be an integer for number of minutes,
/// ISO-8601 duration, or RFC3339 date.
pub fn parse_minutes_duration_or_date(
  sys: &impl sys_traits::SystemTimeNow,
  s: &str,
) -> Result<NewestDependencyDate, ParseDateOrDurationError> {
  if s == "0" {
    // consider 0 as disabled in order to not cause
    // issues when a user's clock is wrong
    Ok(NewestDependencyDate::Disabled)
  } else {
    parse_enabled_minutes_duration_or_date(sys, s)
      .map(NewestDependencyDate::Enabled)
  }
}

fn parse_enabled_minutes_duration_or_date(
  sys: &impl sys_traits::SystemTimeNow,
  s: &str,
) -> Result<chrono::DateTime<chrono::Utc>, ParseDateOrDurationError> {
  if s.chars().all(|c| c.is_ascii_digit()) {
    let now = chrono::DateTime::<chrono::Utc>::from(sys.sys_time_now());
    let minutes: i64 = s
      .parse()
      .map_err(ParseDateOrDurationError::InvalidMinutes)?;
    return Ok(now - chrono::Duration::minutes(minutes));
  }

  let datetime_parse_err = match DateTime::parse_from_rfc3339(s) {
    Ok(dt) => return Ok(dt.with_timezone(&chrono::Utc)),
    Err(err) => err,
  };
  // accept offsets without colon (e.g., +0900) and optional seconds
  if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z")
    .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M%z"))
  {
    return Ok(dt.with_timezone(&chrono::Utc));
  }
  // accept simple date format (YYYY-MM-DD) and treat as midnight UTC
  if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
    return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
  }
  // try duration
  match parse_iso8601_duration(s) {
    Ok(duration) => {
      let now = chrono::DateTime::<chrono::Utc>::from(sys.sys_time_now());
      Ok(now - duration)
    }
    Err(ParseIso8601DurationError::MissingP) => Err(
      ParseDateOrDurationError::InvalidDateTime(datetime_parse_err),
    ),
    Err(err) => Err(ParseDateOrDurationError::InvalidDuration(err)),
  }
}

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
fn parse_iso8601_duration(
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

  // weeks-only short form: PnW
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
      i += 1; // skip '.'
      frac_start = i;
      while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        frac_len += 1;
      }
      if frac_len == 0 {
        return Err(ParseIso8601DurationError::ExpectedFraction);
      }
    }

    // end of the integer slice (just before '.' if present)
    let int_end = if frac_len > 0 { frac_start - 1 } else { i };

    if i >= bytes.len() {
      return Err(ParseIso8601DurationError::MissingUnit);
    }
    let mut unit = bytes[i] as char;
    unit.make_ascii_uppercase();
    i += 1;

    // integral value
    let int_val: i64 = s[start..int_end]
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
          let frac_str = &s[frac_start..(frac_start + frac_len)];
          // take up to 9 digits for nanoseconds, right-pad with zeros
          let n = frac_str.chars().take(9).collect::<String>();
          let scale = 9 - n.len();
          let base: i64 = n
            .parse()
            .map_err(|_| ParseIso8601DurationError::InvalidFractionalSeconds)?;
          let nanos = base
            .checked_mul(10_i64.pow(scale as u32))
            .ok_or(ParseIso8601DurationError::Overflow)?;
          d = d
            .checked_add(&Duration::nanoseconds(nanos))
            .ok_or(ParseIso8601DurationError::Overflow)?;
        }
        d
      }

      // any other time-unit with a fraction is invalid because only seconds allow fractions
      (true, _, f) if f > 0 => {
        return Err(ParseIso8601DurationError::FractionalNotAllowed);
      }

      // unknown/invalid unit in time section (without fraction)
      (true, _, _) => return Err(ParseIso8601DurationError::InvalidUnit),

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
  use std::time::SystemTime;

  use chrono::TimeZone;
  use chrono::Utc;

  use super::*;

  #[cfg(windows)]
  #[test]
  fn is_skippable_io_error_win_invalid_filename() {
    let error = std::io::Error::from_raw_os_error(123);
    assert!(super::is_skippable_io_error(&error));
  }

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

  #[test]
  fn test_parse_minutes_duration_or_date() {
    struct TestEnv;

    impl sys_traits::SystemTimeNow for TestEnv {
      fn sys_time_now(&self) -> SystemTime {
        let datetime = Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap();
        SystemTime::from(datetime)
      }
    }

    // zero becomes disabled to prevent issues with clock drift
    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "0").unwrap(),
      NewestDependencyDate::Disabled
    );

    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "120").unwrap(),
      NewestDependencyDate::Enabled(
        Utc.with_ymd_and_hms(2025, 5, 31, 22, 0, 0).unwrap()
      )
    );

    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "2025-01-01").unwrap(),
      NewestDependencyDate::Enabled(
        Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()
      )
    );

    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "2025-01-01").unwrap(),
      NewestDependencyDate::Enabled(
        Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()
      )
    );

    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "2025-09-16T12:50:10+00:00")
        .unwrap(),
      NewestDependencyDate::Enabled(
        Utc.with_ymd_and_hms(2025, 9, 16, 12, 50, 10).unwrap()
      )
    );

    assert_eq!(
      parse_minutes_duration_or_date(&TestEnv, "P2D").unwrap(),
      NewestDependencyDate::Enabled(
        Utc.with_ymd_and_hms(2025, 5, 30, 0, 0, 0).unwrap()
      )
    );
  }
}
