// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use std::path::PathBuf;

/// Attempts to convert a specifier to a file path. By default, uses the Url
/// crate's `to_file_path()` method, but falls back to try and resolve unix-style
/// paths on Windows.
pub fn specifier_to_file_path(
  specifier: &ModuleSpecifier,
) -> Result<PathBuf, AnyError> {
  dbg!(&specifier);
  dbg!(&specifier.as_str());

  let result = if !specifier.as_str().starts_with("file:///") {
    Err(())
  } else if cfg!(windows) {
    match specifier.to_file_path() {
      Ok(path) => Ok(path),
      Err(()) => {
        // This might be a unix-style path which is used in the tests even on Windows.
        // Attempt to see if we can convert it to a `PathBuf`. This code should be removed
        // once/if https://github.com/servo/rust-url/issues/730 is implemented.
        if specifier.scheme() == "file"
          && specifier.host().is_none()
          && specifier.port().is_none()
          && specifier.path_segments().is_some()
        {
          let path_str = specifier.path();
          match String::from_utf8(
            percent_encoding::percent_decode(path_str.as_bytes()).collect(),
          ) {
            Ok(path_str) => Ok(PathBuf::from(path_str)),
            Err(_) => Err(()),
          }
        } else {
          Err(())
        }
      }
    }
  } else {
    specifier.to_file_path()
  };
  match result {
    Ok(path) => Ok(path),
    Err(()) => Err(uri_error(format!(
      "Invalid file path.\n  Specifier: {specifier}"
    ))),
  }
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn test_specifier_to_file_path() {
    run_success_test("file:///", "/");
    run_success_test("file:///test", "/test");
    run_success_test("file:///dir/test/test.txt", "/dir/test/test.txt");
    run_success_test(
      "file:///dir/test%20test/test.txt",
      "/dir/test test/test.txt",
    );
    assert_is_error("file:/");
    assert_is_error("file://");

    fn run_success_test(specifier: &str, expected_path: &str) {
      let result =
        specifier_to_file_path(&ModuleSpecifier::parse(specifier).unwrap())
          .unwrap();
      assert_eq!(result, PathBuf::from(expected_path));
    }
    fn assert_is_error(specifier: &str) {
      let result =
        specifier_to_file_path(&ModuleSpecifier::parse(specifier).unwrap());
      assert!(result.is_err());
    }
  }
}
