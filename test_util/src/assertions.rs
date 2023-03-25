// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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

#[macro_export]
macro_rules! failed_position {
  () => {
    $crate::assertions::failed_position(std::file!())
  };
}

pub fn failed_position(unwrap_filename: &str) -> String {
  let backtrace = backtrace::Backtrace::new();

  let mut found_file = false;
  for frame in backtrace.frames() {
    for symbol in frame.symbols() {
      if let Some(filename) = symbol.filename() {
        if !found_file {
          found_file = filename.to_string_lossy().ends_with(unwrap_filename);
        } else if !filename.to_string_lossy().ends_with(unwrap_filename) {
          let line_num = symbol.lineno().unwrap_or(0);
          let line_col = symbol.colno().unwrap_or(0);
          return format!("{}:{}:{}", filename.display(), line_num, line_col);
        }
      }
    }
  }

  "<unknown>".to_string()
}
