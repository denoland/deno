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
macro_rules! assert_output_text {
  ($output:expr, $expected:expr) => {
    let expected_text = $expected;
    let actual = $output.text();

    if !expected_text.contains("[WILDCARD]") {
      assert_eq!(actual, expected_text)
    } else if !test_util::wildcard_match(&expected_text, actual) {
      println!("OUTPUT START\n{}\nOUTPUT END", actual);
      println!("EXPECTED START\n{expected_text}\nEXPECTED END");
      panic!("pattern match failed");
    }
  };
}

#[macro_export]
macro_rules! assert_output_file {
  ($output:expr, $file_path:expr) => {
    let output = &$output;
    let output_path = output.testdata_dir().join($file_path);
    println!("output path {}", output_path.display());
    let expected_text =
      std::fs::read_to_string(&output_path).unwrap_or_else(|err| {
        panic!("failed loading {}\n\n{err:#}", output_path.display())
      });
    test_util::assert_output_text!(output, expected_text);
  };
}

#[macro_export]
macro_rules! assert_exit_code {
  ($output:expr, $exit_code:expr) => {
    let output = &$output;
    let actual = output.text();
    let expected_exit_code = $exit_code;
    let actual_exit_code = output.exit_code();

    if let Some(exit_code) = &actual_exit_code {
      if *exit_code != expected_exit_code {
        println!("OUTPUT\n{actual}\nOUTPUT");
        panic!(
          "bad exit code, expected: {:?}, actual: {:?}",
          expected_exit_code, exit_code
        );
      }
    } else {
      println!("OUTPUT\n{actual}\nOUTPUT");
      if let Some(signal) = output.signal() {
        panic!(
          "process terminated by signal, expected exit code: {:?}, actual signal: {:?}",
          actual_exit_code, signal,
        );
      } else {
        panic!("process terminated without status code on non unix platform, expected exit code: {:?}", actual_exit_code);
      }
    }
  };
}
