// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;

use crate::colors;

#[macro_export]
macro_rules! assert_starts_with {
  ($string:expr, $($test:expr),+) => {
    let string = $string; // This might be a function call or something
    if !($(string.starts_with($test))||+) {
      panic!("{:?} does not start with {:?}", string, [$($test),+]);
    }
  }
}

#[macro_export]
macro_rules! assert_ends_with {
  ($left:expr, $right:expr $(,)?) => {
    match (&$left, &$right) {
      (actual, expected) => {
        let actual = if expected.len() > actual.len() {
          actual
        } else {
          &actual[actual.len() - expected.len()..]
        };
        pretty_assertions::assert_eq!(
          actual,
          *expected,
          "should end with expected."
        );
      }
    }
  };
}

#[macro_export]
macro_rules! assert_contains {
  ($string:expr, $($test:expr),+ $(,)?) => {
    let string = &$string; // This might be a function call or something
    if !($(string.contains($test))||+) {
      panic!("{:?} does not contain any of {:?}", string, [$($test),+]);
    }
  }
}

#[macro_export]
macro_rules! assert_not_contains {
  ($string:expr, $($test:expr),+ $(,)?) => {
    let string = &$string; // This might be a function call or something
    if !($(!string.contains($test))||+) {
      panic!("{:?} contained {:?}", string, [$($test),+]);
    }
  }
}

#[track_caller]
pub fn assert_wildcard_match(actual: &str, expected: &str) {
  assert_wildcard_match_with_logger(actual, expected, &mut std::io::stderr())
}

#[track_caller]
pub fn assert_wildcard_match_with_logger(
  actual: &str,
  expected: &str,
  logger: &mut dyn Write,
) {
  if !expected.contains("[WILD")
    && !expected.contains("[UNORDERED_START]")
    && !expected.contains("[#")
  {
    pretty_assertions::assert_eq!(actual, expected);
  } else {
    match crate::wildcard_match_detailed(expected, actual) {
      crate::WildcardMatchResult::Success => {
        // ignore
      }
      crate::WildcardMatchResult::Fail(debug_output) => {
        writeln!(
          logger,
          "{}{}{}",
          colors::bold("-- "),
          colors::bold_red("OUTPUT"),
          colors::bold(" START --"),
        )
        .unwrap();
        writeln!(logger, "{}", actual).unwrap();
        writeln!(logger, "{}", colors::bold("-- OUTPUT END --")).unwrap();
        writeln!(
          logger,
          "{}{}{}",
          colors::bold("-- "),
          colors::bold_green("EXPECTED"),
          colors::bold(" START --"),
        )
        .unwrap();
        writeln!(logger, "{}", expected).unwrap();
        writeln!(logger, "{}", colors::bold("-- EXPECTED END --")).unwrap();
        writeln!(
          logger,
          "{}{}{}",
          colors::bold("-- "),
          colors::bold_blue("DEBUG"),
          colors::bold(" START --"),
        )
        .unwrap();
        writeln!(logger, "{debug_output}").unwrap();
        writeln!(logger, "{}", colors::bold("-- DEBUG END --")).unwrap();
        panic!("pattern match failed");
      }
    }
  }
}

/// Asserts that the actual `serde_json::Value` is equal to the expected `serde_json::Value`, but
/// only for the keys present in the expected value.
///
/// # Example
///
/// ```
/// # use serde_json::json;
/// # use test_server::assertions::assert_json_subset;
/// assert_json_subset(json!({"a": 1, "b": 2}), json!({"a": 1}));
///
/// // Arrays are compared element by element
/// assert_json_subset(json!([{ "a": 1, "b": 2 }, {}]), json!([{"a": 1}, {}]));
/// ```
#[track_caller]
pub fn assert_json_subset(
  actual: serde_json::Value,
  expected: serde_json::Value,
) {
  match (actual, expected) {
    (
      serde_json::Value::Object(actual),
      serde_json::Value::Object(expected),
    ) => {
      for (k, v) in expected.iter() {
        let Some(actual_v) = actual.get(k) else {
          panic!("Key {k:?} not found in actual value ({actual:#?})");
        };
        assert_json_subset(actual_v.clone(), v.clone());
      }
    }
    (serde_json::Value::Array(actual), serde_json::Value::Array(expected)) => {
      for (i, v) in expected.iter().enumerate() {
        assert_json_subset(actual[i].clone(), v.clone());
      }
    }
    (actual, expected) => {
      assert_eq!(actual, expected);
    }
  }
}
