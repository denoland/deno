// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hasher;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use crate::eprintln;

/// A fast hasher for computing a combined hash of file contents.
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

  /// Hash a single file's contents. Skips if file doesn't exist.
  pub fn hash_file(&mut self, path: impl AsRef<Path>) -> &mut Self {
    if let Ok(mut file) = std::fs::File::open(path.as_ref()) {
      self.hash_reader(&mut file);
    }
    self
  }

  /// Recursively hash all file contents in a directory (sorted for
  /// determinism). Skips if directory doesn't exist.
  pub fn hash_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
    let path = path.as_ref();
    let mut entries = Vec::new();
    collect_entries_recursive(path, &mut entries);
    entries.sort();
    for entry_path in &entries {
      if let Ok(rel) = entry_path.strip_prefix(path) {
        self.0.write(rel.as_os_str().as_encoded_bytes());
      }
      if let Ok(mut file) = std::fs::File::open(entry_path) {
        self.hash_reader(&mut file);
      }
    }
    self
  }

  fn hash_reader(&mut self, reader: &mut impl Read) {
    let mut buf = [0u8; 8192];
    loop {
      match reader.read(&mut buf) {
        Ok(0) | Err(_) => break,
        Ok(n) => self.0.write(&buf[..n]),
      }
    }
  }

  pub fn finish(&self) -> u64 {
    self.0.finish()
  }
}

fn collect_entries_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
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
      out.push(entry.path());
    }
  }
}

// repo at https://github.com/denoland/hashy
const HASHY_URL: &str = "https://hashy.deno.deno.net";

pub enum CiHashStatus {
  /// Not on CI — always run tests.
  Run,
  /// Hash found in hashy — skip tests.
  Skip,
  /// Hash not found — run tests, then call `commit()` on success.
  RunThenCommit(CiHashPending),
}

pub struct CiHashPending {
  key: String,
}

impl CiHashPending {
  /// Mark the hash as known-good after tests pass.
  pub fn commit(self) {
    let url = format!("{}/hashes/{}", HASHY_URL, self.key);
    match std::process::Command::new("curl")
      .args(["-sf", "-X", "PUT", "--max-time", "5", &url])
      .output()
    {
      Ok(output) if output.status.success() => {
        eprintln!("hashy: committed hash {}", self.key);
      }
      Ok(output) => {
        eprintln!(
          "hashy: failed to commit hash {} (exit code {:?})",
          self.key,
          output.status.code()
        );
      }
      Err(e) => {
        eprintln!("hashy: failed to run curl: {e}");
      }
    }
  }
}

/// Check if tests can be skipped on CI by comparing input hashes
/// against the hashy service.
///
/// `name` is used for the hash key and log messages (e.g. "specs",
/// "unit"). `configure` receives an `InputHasher` to add whatever files/dirs
/// are relevant.
///
/// Returns a `CiHashStatus` indicating whether to skip, run, or
/// run-then-commit.
///
/// ```ignore
/// let ci_hash = test_util::hash::check_ci_hash("specs", |hasher| {
///     hasher
///         .hash_dir(tests.join("specs"))
///         .hash_file(deno_exe_path());
/// });
/// if matches!(ci_hash, CiHashStatus::Skip) {
///     return;
/// }
/// // ... run tests ...
/// if let CiHashStatus::RunThenCommit(pending) = ci_hash {
///     pending.commit();
/// }
/// ```
pub fn check_ci_hash(
  name: &str,
  configure: impl FnOnce(&mut InputHasher),
) -> CiHashStatus {
  if !*crate::IS_CI {
    return CiHashStatus::Run;
  }

  let start = std::time::Instant::now();
  let mut hasher = InputHasher::new_with_cli_args();
  configure(&mut hasher);
  let hash = format!("{:016x}", hasher.finish());
  let key = format!("{name}_{hash}");

  eprintln!("ci hash took {}ms", start.elapsed().as_millis());

  // On main/tag builds, always run tests but still commit on success
  // to seed the cache for PR builds.
  if is_main_or_tag() {
    eprintln!("hashy: main/tag build, running tests (will commit on success)");
    return CiHashStatus::RunThenCommit(CiHashPending { key });
  }

  let url = format!("{}/hashes/{}", HASHY_URL, key);
  match std::process::Command::new("curl")
    .args(["-sf", "--max-time", "5", &url])
    .output()
  {
    Ok(output) if output.status.success() => {
      eprintln!("hashy: {name} hash found ({key}), skipping");
      CiHashStatus::Skip
    }
    Ok(_) => {
      eprintln!("hashy: {name} hash not found ({key}), will run tests");
      CiHashStatus::RunThenCommit(CiHashPending { key })
    }
    Err(e) => {
      eprintln!("hashy: failed to check hash, running tests: {e}");
      CiHashStatus::Run
    }
  }
}

fn is_main_or_tag() -> bool {
  std::env::var("GITHUB_REF")
    .map(|r| r == "refs/heads/main" || r.starts_with("refs/tags/"))
    .unwrap_or(false)
}
