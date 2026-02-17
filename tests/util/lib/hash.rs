// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;

use crate::eprintln;

/// A fast hasher for computing a combined hash of files and directories.
/// Uses xxhash64 for speed.
#[derive(Default)]
pub struct InputHasher(twox_hash::XxHash64);

impl InputHasher {
  /// Hash a single file's contents. Skips if file doesn't exist.
  pub fn hash_file(&mut self, path: impl AsRef<Path>) -> &mut Self {
    let path = path.as_ref();
    if let Ok(bytes) = std::fs::read(path) {
      self.0.write(path.to_string_lossy().as_bytes());
      self.0.write(&bytes);
    }
    self
  }

  /// Recursively hash all files in a directory (sorted for determinism).
  /// Skips if directory doesn't exist.
  pub fn hash_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
    let path = path.as_ref();
    let mut files = Vec::new();
    collect_files_recursive(path, &mut files);
    files.sort();
    for file in &files {
      if let Ok(rel) = file.strip_prefix(path) {
        self.0.write(rel.to_string_lossy().as_bytes());
      }
      if let Ok(bytes) = std::fs::read(file) {
        self.0.write(&bytes);
      }
    }
    self
  }

  pub fn finish(&self) -> u64 {
    self.0.finish()
  }
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect_files_recursive(&path, out);
    } else {
      out.push(path);
    }
  }
}

/// Check if tests can be skipped on CI by comparing input hashes.
///
/// `name` is used for the hash file name and log messages (e.g. "specs",
/// "unit"). `configure` receives an `InputHasher` to add whatever files/dirs
/// are relevant.
///
/// Returns `true` if the hash is unchanged and tests should be skipped.
///
/// ```ignore
/// if test_util::hash::should_skip_on_ci("specs", |hasher| {
///     hasher
///         .hash_dir(tests.join("specs"))
///         .hash_file(deno_exe_path());
/// }) {
///     return;
/// }
/// ```
pub fn should_skip_on_ci(
  name: &str,
  configure: impl FnOnce(&mut InputHasher),
) -> bool {
  if !*crate::IS_CI {
    return false;
  }

  let start = std::time::Instant::now();
  let hash_path = crate::target_dir()
    .join(format!("{name}_input_hash"))
    .to_path_buf();
  let mut hasher = InputHasher::default();
  configure(&mut hasher);
  let new_hash = hasher.finish().to_string();

  eprintln!("ci hash took {}ms", start.elapsed().as_millis());

  if let Ok(old_hash) = std::fs::read_to_string(&hash_path)
    && old_hash.trim() == new_hash
  {
    eprintln!("{name} input hash unchanged ({new_hash}), skipping");
    return true;
  }

  eprintln!("{name} input hash changed, writing new hash ({new_hash})");
  std::fs::write(&hash_path, &new_hash).ok();
  false
}
