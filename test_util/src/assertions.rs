// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
  if !expected.contains("[WILDCARD]") {
    pretty_assertions::assert_eq!(actual, expected);
  } else if !crate::wildcard_match(expected, actual) {
    println!("OUTPUT START\n{actual}\nOUTPUT END");
    println!("EXPECTED START\n{expected}\nEXPECTED END");
    panic!("pattern match failed");
  }
}
