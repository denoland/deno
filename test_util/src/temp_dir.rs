use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Context;
use once_cell::sync::OnceCell;

static TEMP_DIR_SESSION: OnceCell<TempDirSession> = OnceCell::new();

struct TempDirSession {
  default_prefix: String,
  counter: AtomicU32,
}

/// For creating temporary directories in tests.
///
/// This was done because `tempfiles::TempDir` was very slow on Windows.
///
/// Note: Do not use this in actual code as this does not protect against
/// "insecure temporary file" security vulnerabilities.
#[derive(Clone)]
pub struct TempDir(Arc<TempDirInner>);

struct TempDirInner(PathBuf);

impl Drop for TempDirInner {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

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

  fn new_inner(parent_dir: &Path, prefix: Option<&str>) -> Self {
    let session = TEMP_DIR_SESSION.get_or_init(|| {
      let default_prefix = format!(
        "deno-cli-test-{}",
        SystemTime::now()
          .duration_since(SystemTime::UNIX_EPOCH)
          .unwrap()
          .as_millis()
      );
      TempDirSession {
        default_prefix,
        counter: Default::default(),
      }
    });
    Self({
      let count = session.counter.fetch_add(1, Ordering::SeqCst);
      let path = parent_dir.join(format!(
        "{}{}-{}",
        prefix.unwrap_or(""),
        session.default_prefix,
        count,
      ));
      std::fs::create_dir_all(&path)
        .with_context(|| format!("Error creating temp dir: {}", path.display()))
        .unwrap();
      Arc::new(TempDirInner(path))
    })
  }

  pub fn path(&self) -> &Path {
    let inner = &self.0;
    inner.0.as_path()
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
