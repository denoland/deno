// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::Context;
pub use deno_core::normalize_path;
use std::cell::RefCell;
use std::io::Error;
use std::path::{Path, PathBuf};

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  let mut canonicalized_path = path.canonicalize()?;
  if cfg!(windows) {
    canonicalized_path = PathBuf::from(
      canonicalized_path
        .display()
        .to_string()
        .trim_start_matches("\\\\?\\"),
    );
  }
  Ok(canonicalized_path)
}

thread_local!(static CACHED_CWD: RefCell<Option<PathBuf>> = Default::default());

pub fn cached_cwd() -> Result<PathBuf, AnyError> {
  CACHED_CWD.with(|cached| {
    if let Some(cached) = &*cached.borrow() {
      return Ok(cached.clone());
    }
    let cwd = std::env::current_dir()?;
    cached.borrow_mut().replace(cwd.clone());
    Ok(cwd)
  })
}

pub fn cached_chdir(cwd: &PathBuf) -> Result<(), AnyError> {
  std::env::set_current_dir(cwd)?;
  CACHED_CWD.with(|cached| cached.take());
  Ok(())
}

pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  let resolved_path = if path.is_absolute() {
    path.to_owned()
  } else {
    let cwd =
      cached_cwd().context("Failed to get current working directory")?;
    cwd.join(path)
  };

  Ok(normalize_path(&resolved_path))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = current_dir().unwrap();
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

  // TODO: Get a good expected value here for Windows.
  #[cfg(not(windows))]
  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("/a");
    assert_eq!(resolve_from_cwd(expected).unwrap(), expected);
  }
}
