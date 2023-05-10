// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use lsp_types::Url;

/// For creating temporary directories in tests.
///
/// This was done because `tempfiles::TempDir` was very slow on Windows.
///
/// Note: Do not use this in actual code as this does not protect against
/// "insecure temporary file" security vulnerabilities.
#[derive(Clone)]
pub struct TempDir(Arc<tempfile::TempDir>);

impl Default for TempDir {
  fn default() -> Self {
    Self::new()
  }
}

impl TempDir {
  pub fn new() -> Self {
    Self::new_inner(&std::env::temp_dir(), None)
  }

  pub fn new_in(path: &Path) -> Self {
    Self::new_inner(path, None)
  }

  pub fn new_with_prefix(prefix: &str) -> Self {
    Self::new_inner(&std::env::temp_dir(), Some(prefix))
  }

  /// Create a new temporary directory with the given prefix as part of its name, if specified.
  fn new_inner(parent_dir: &Path, prefix: Option<&str>) -> Self {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix.unwrap_or("deno-cli-test"));
    let dir = builder
      .tempdir_in(parent_dir)
      .expect("Failed to create a temporary directory");
    Self(dir.into())
  }

  pub fn uri(&self) -> Url {
    Url::from_directory_path(self.path()).unwrap()
  }

  pub fn path(&self) -> &Path {
    let inner = &self.0;
    inner.path()
  }

  pub fn create_dir_all(&self, path: impl AsRef<Path>) {
    fs::create_dir_all(self.path().join(path)).unwrap();
  }

  pub fn read_to_string(&self, path: impl AsRef<Path>) -> String {
    let file_path = self.path().join(path);
    fs::read_to_string(&file_path)
      .with_context(|| format!("Could not find file: {}", file_path.display()))
      .unwrap()
  }

  pub fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) {
    fs::rename(self.path().join(from), self.path().join(to)).unwrap();
  }

  pub fn write(&self, path: impl AsRef<Path>, text: impl AsRef<str>) {
    fs::write(self.path().join(path), text.as_ref()).unwrap();
  }
}
