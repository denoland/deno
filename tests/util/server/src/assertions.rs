// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
  if !expected.contains("[WILD") && !expected.contains("[UNORDERED_START]") {
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
