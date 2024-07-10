// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
pub use deno_core::normalize_path;
use std::path::Path;
use std::path::PathBuf;

#[inline]
pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  if path.is_absolute() {
    Ok(normalize_path(path))
  } else {
    #[allow(clippy::disallowed_methods)]
    let cwd = std::env::current_dir()
      .context("Failed to get current working directory")?;
    Ok(normalize_path(cwd.join(path)))
  }
}

/// Attempts to convert a specifier to a file path. By default, uses the Url
/// crate's `to_file_path()` method, but falls back to try and resolve unix-style
/// paths on Windows.
pub fn specifier_to_file_path(
  specifier: &ModuleSpecifier,
) -> Result<PathBuf, AnyError> {
  let result = if specifier.scheme() != "file" {
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
mod tests {
  use super::*;

  fn current_dir() -> PathBuf {
    #[allow(clippy::disallowed_methods)]
    std::env::current_dir().unwrap()
  }

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = current_dir();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = current_dir();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = current_dir();
    assert_eq!(resolve_from_cwd(Path::new("a/..")).unwrap(), cwd);
  }

  #[test]
  fn test_normalize_path() {
    assert_eq!(normalize_path(Path::new("a/../b")), PathBuf::from("b"));
    assert_eq!(normalize_path(Path::new("a/./b/")), PathBuf::from("a/b/"));
    assert_eq!(
      normalize_path(Path::new("a/./b/../c")),
      PathBuf::from("a/c")
    );

    if cfg!(windows) {
      assert_eq!(
        normalize_path(Path::new("C:\\a\\.\\b\\..\\c")),
        PathBuf::from("C:\\a\\c")
      );
    }
  }

  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("a");
    let cwd = current_dir();
    let absolute_expected = cwd.join(expected);
    assert_eq!(resolve_from_cwd(expected).unwrap(), absolute_expected);
  }

  #[test]
  fn test_specifier_to_file_path() {
    run_success_test("file:///", "/");
    run_success_test("file:///test", "/test");
    run_success_test("file:///dir/test/test.txt", "/dir/test/test.txt");
    run_success_test(
      "file:///dir/test%20test/test.txt",
      "/dir/test test/test.txt",
    );

    fn run_success_test(specifier: &str, expected_path: &str) {
      let result =
        specifier_to_file_path(&ModuleSpecifier::parse(specifier).unwrap())
          .unwrap();
      assert_eq!(result, PathBuf::from(expected_path));
    }
  }
}
