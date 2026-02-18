// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::eprintln;

/// A fast hasher for computing a combined hash of files and directory mtimes.
/// Uses xxhash64 for speed.
pub struct InputHasher(twox_hash::XxHash64);

impl InputHasher {
  pub fn new_with_cli_args() -> Self {
    let mut hasher = Self(Default::default());
    for arg in std::env::args() {
      hasher.0.write(arg.as_bytes());
    }
    hasher
  }

  /// Hash a single file's mtime. Skips if file doesn't exist.
  pub fn hash_file(&mut self, path: impl AsRef<Path>) -> &mut Self {
    if let Ok(meta) = std::fs::metadata(path.as_ref()) {
      self.hash_mtime(meta.modified().ok());
    }
    self
  }

  /// Recursively hash all file mtimes in a directory (sorted for determinism).
  /// Skips if directory doesn't exist.
  pub fn hash_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
    let path = path.as_ref();
    let mut entries = Vec::new();
    collect_entries_recursive(path, &mut entries);
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (entry_path, mtime) in &entries {
      if let Ok(rel) = entry_path.strip_prefix(path) {
        self.0.write(rel.as_os_str().as_encoded_bytes());
      }
      self.hash_mtime(*mtime);
      eprintln!("{} {:?}", entry_path.display(), mtime);
    }
    self
  }

  fn hash_mtime(&mut self, mtime: Option<SystemTime>) {
    if let Some(mtime) = mtime
      && let Ok(d) = mtime.duration_since(SystemTime::UNIX_EPOCH)
    {
      self.0.write_u64(d.as_secs());
    }
  }

  pub fn finish(&self) -> u64 {
    self.0.finish()
  }
}

fn collect_entries_recursive(
  dir: &Path,
  out: &mut Vec<(PathBuf, Option<SystemTime>)>,
) {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let file_type = match entry.file_type() {
      Ok(ft) => ft,
      Err(_) => continue,
    };
    if file_type.is_dir() {
      collect_entries_recursive(&entry.path(), out);
    } else {
      let mtime = entry.metadata().ok().and_then(|m| m.modified().ok());
      out.push((entry.path(), mtime));
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
  let mut hasher = InputHasher::new_with_cli_args();
  configure(&mut hasher);
  let new_hash = hasher.finish().to_string();

  eprintln!("ci hash took {}ms", start.elapsed().as_millis());

  let maybe_old_hash = std::fs::read_to_string(&hash_path).ok();
  let maybe_old_hash = maybe_old_hash.as_ref().map(|h| h.trim());
  if maybe_old_hash == Some(&new_hash) {
    eprintln!("{name} input hash unchanged ({new_hash}), skipping");
    return true;
  }

  eprintln!(
    "{name} input hash changed from {maybe_old_hash:?}, writing new hash ({new_hash})"
  );
  std::fs::write(&hash_path, &new_hash).ok();
  false
}
