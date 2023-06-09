// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use lsp_types::Url;

enum TempDirInner {
  TempDir(tempfile::TempDir),
  Path(PathBuf),
  Symlinked {
    symlink: Arc<TempDirInner>,
    target: Arc<TempDirInner>,
  },
}

impl TempDirInner {
  pub fn path(&self) -> &Path {
    match self {
      Self::Path(path) => path.as_path(),
      Self::TempDir(dir) => dir.path(),
      Self::Symlinked { symlink, .. } => symlink.path(),
    }
  }

  pub fn target_path(&self) -> &Path {
    match self {
      TempDirInner::Symlinked { target, .. } => target.target_path(),
      _ => self.path(),
    }
  }
}

impl Drop for TempDirInner {
  fn drop(&mut self) {
    if let Self::Path(path) = self {
      _ = fs::remove_dir_all(path);
    }
  }
}

/// For creating temporary directories in tests.
///
/// This was done because `tempfiles::TempDir` was very slow on Windows.
///
/// Note: Do not use this in actual code as this does not protect against
/// "insecure temporary file" security vulnerabilities.
#[derive(Clone)]
pub struct TempDir(Arc<TempDirInner>);

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

  pub fn new_with_path(path: &Path) -> Self {
    Self(Arc::new(TempDirInner::Path(path.to_path_buf())))
  }

  pub fn new_symlinked(target: TempDir) -> Self {
    let target_path = target.path();
    let path = target_path.parent().unwrap().join(format!(
      "{}_symlinked",
      target_path.file_name().unwrap().to_str().unwrap()
    ));
    target.symlink_dir(target.path(), &path);
    TempDir(Arc::new(TempDirInner::Symlinked {
      target: target.0,
      symlink: Self::new_with_path(&path).0,
    }))
  }

  /// Create a new temporary directory with the given prefix as part of its name, if specified.
  fn new_inner(parent_dir: &Path, prefix: Option<&str>) -> Self {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix.unwrap_or("deno-cli-test"));
    let dir = builder
      .tempdir_in(parent_dir)
      .expect("Failed to create a temporary directory");
    Self(Arc::new(TempDirInner::TempDir(dir)))
  }

  pub fn uri(&self) -> Url {
    Url::from_directory_path(self.path()).unwrap()
  }

  pub fn path(&self) -> &Path {
    self.0.path()
  }

  /// The resolved final target path if this is a symlink.
  pub fn target_path(&self) -> &Path {
    self.0.target_path()
  }

  pub fn create_dir_all(&self, path: impl AsRef<Path>) {
    fs::create_dir_all(self.target_path().join(path)).unwrap();
  }

  pub fn remove_file(&self, path: impl AsRef<Path>) {
    fs::remove_file(self.target_path().join(path)).unwrap();
  }

  pub fn remove_dir_all(&self, path: impl AsRef<Path>) {
    fs::remove_dir_all(self.target_path().join(path)).unwrap();
  }

  pub fn read_to_string(&self, path: impl AsRef<Path>) -> String {
    let file_path = self.target_path().join(path);
    fs::read_to_string(&file_path)
      .with_context(|| format!("Could not find file: {}", file_path.display()))
      .unwrap()
  }

  pub fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) {
    fs::rename(self.target_path().join(from), self.path().join(to)).unwrap();
  }

  pub fn write(&self, path: impl AsRef<Path>, text: impl AsRef<str>) {
    fs::write(self.target_path().join(path), text.as_ref()).unwrap();
  }

  pub fn symlink_dir(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(self.path().join(oldpath), self.path().join(newpath)).unwrap();
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::symlink_dir;
      symlink_dir(self.path().join(oldpath), self.path().join(newpath))
        .unwrap();
    }
  }

  pub fn symlink_file(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(self.path().join(oldpath), self.path().join(newpath)).unwrap();
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::symlink_file;
      symlink_file(self.path().join(oldpath), self.path().join(newpath))
        .unwrap();
    }
  }
}
