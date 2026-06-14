// Copyright 2018-2026 the Deno authors. MIT license.

//! Input-based caching for `deno task`.
//!
//! When a task declares `files`, Deno computes a fingerprint over the
//! contents of those files, the command string (including any arguments
//! appended on the CLI), and the values of any environment variables
//! listed in `env`. If a previous run wrote a matching fingerprint to
//! disk, the task is skipped.
//!
//! This is the first pass: no output restoration, no dependency
//! fingerprint propagation, no remote cache. The on-disk format is a
//! single hex-encoded `u64` per task; that lets us evolve the schema
//! later without migrating anything.
//!
//! Caveats of this first pass (see the deferred-work list on the PR):
//!
//! - Outputs are not tracked or restored. A hit skips execution entirely,
//!   so if a previous run produced artifacts that were since deleted
//!   (`dist/` removed, inputs unchanged), the task still hits and the
//!   artifacts are *not* regenerated. Until output restoration lands,
//!   input caching is best suited to tasks that are pure with respect to
//!   their declared inputs.
//! - The fingerprint is captured before the run and stored afterwards, so
//!   a task that writes into its own `files` (formatters, codegen) changes
//!   its inputs as a side effect and will never match on the next run. It
//!   stays correct, just never caches.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_lib::util::hash::FastInsecureHasher;

use crate::sys::CliSys;

/// Result of consulting the cache for a task.
pub enum CacheLookup {
  /// The task is eligible for caching and the stored fingerprint matches,
  /// so the task may be skipped.
  Hit,
  /// The task is eligible for caching but no stored fingerprint matches.
  /// Caller should run the task and then call [`TaskCache::store`] with
  /// the returned fingerprint.
  Miss(u64),
  /// The task does not opt in to caching (no `files` declared).
  NotCacheable,
}

pub struct TaskCache {
  dir: PathBuf,
}

impl TaskCache {
  pub fn new(deno_dir_root: &Path) -> Self {
    Self {
      dir: deno_dir_root.join("task_cache_v1"),
    }
  }

  /// Compute the fingerprint for a task and check whether it matches
  /// what's on disk.
  pub fn lookup(&self, key: &TaskCacheKey<'_>) -> CacheLookup {
    if key.files.is_empty() {
      return CacheLookup::NotCacheable;
    }
    let Some(fingerprint) = compute_fingerprint(key) else {
      return CacheLookup::NotCacheable;
    };
    match std::fs::read_to_string(self.entry_path(key)) {
      Ok(stored) if stored.trim() == format!("{:016x}", fingerprint) => {
        CacheLookup::Hit
      }
      _ => CacheLookup::Miss(fingerprint),
    }
  }

  /// Persist a fingerprint for a successfully completed task.
  pub fn store(&self, key: &TaskCacheKey<'_>, fingerprint: u64) {
    if let Err(err) = std::fs::create_dir_all(&self.dir) {
      log::debug!("failed to create task cache dir: {err}");
      return;
    }
    let path = self.entry_path(key);
    if let Err(err) = std::fs::write(&path, format!("{:016x}\n", fingerprint)) {
      log::debug!("failed to write task cache entry {}: {err}", path.display());
    }
  }

  fn entry_path(&self, key: &TaskCacheKey<'_>) -> PathBuf {
    // Hash the identity (package + task name + cwd) so we don't have to
    // worry about path-unsafe characters and so workspace members with
    // identical task names don't collide.
    let mut hasher = FastInsecureHasher::new_deno_versioned();
    hasher.write_str(key.package_name.unwrap_or(""));
    hasher.write_u8(0);
    hasher.write_str(key.task_name);
    hasher.write_u8(0);
    hasher.write(key.cwd.as_os_str().as_encoded_bytes());
    self.dir.join(format!("{:016x}", hasher.finish()))
  }
}

pub struct TaskCacheKey<'a> {
  pub package_name: Option<&'a str>,
  pub task_name: &'a str,
  pub cwd: &'a Path,
  pub command: &'a str,
  /// Arguments appended to the command on the CLI (`deno task build <argv>`).
  /// These change what actually runs, so they must be part of the
  /// fingerprint.
  pub argv: &'a [String],
  pub files: &'a [String],
  pub env_names: &'a [String],
  /// Snapshot of the current environment, looked up at fingerprint time.
  /// Pass the same map the task will execute with.
  pub env: &'a BTreeMap<String, String>,
}

fn compute_fingerprint(key: &TaskCacheKey<'_>) -> Option<u64> {
  let mut hasher = FastInsecureHasher::new_deno_versioned();
  hasher.write_str("deno-task-cache-v1");
  hasher.write_str(key.command);
  // Appended CLI args materially change what runs, so fold them into the
  // fingerprint. Length-prefix to keep the boundaries unambiguous.
  hasher.write_u64(key.argv.len() as u64);
  for arg in key.argv {
    hasher.write_str(arg);
    hasher.write_u8(0);
  }

  // Capture only the env vars the user explicitly named, in a deterministic
  // order. Missing vars hash as the empty string with a distinguishing tag.
  let mut sorted_env = key.env_names.to_vec();
  sorted_env.sort();
  sorted_env.dedup();
  for name in &sorted_env {
    hasher.write_str(name);
    hasher.write_u8(0);
    match key.env.get(name) {
      Some(value) => {
        hasher.write_u8(1);
        hasher.write_str(value);
      }
      None => {
        hasher.write_u8(0);
      }
    }
  }

  // Resolve file globs relative to the task's cwd and hash matching files
  // in sorted order, mixing both the relative path and the contents.
  let patterns = build_file_patterns(key.cwd, key.files)?;
  let mut files = FileCollector::new(|_| true)
    .ignore_git_folder()
    .ignore_node_modules()
    .collect_file_patterns(&CliSys::default(), &patterns);
  files.sort();
  if files.is_empty() {
    // An input glob that matches nothing is almost certainly a config
    // error and would silently make the cache always hit. Bail out so
    // the task runs normally.
    return None;
  }
  for path in &files {
    let rel = path.strip_prefix(key.cwd).unwrap_or(path);
    hasher.write(rel.as_os_str().as_encoded_bytes());
    hasher.write_u8(0);
    match std::fs::read(path) {
      Ok(bytes) => {
        hasher.write_u8(1);
        hasher.write(&bytes);
      }
      Err(_) => {
        hasher.write_u8(0);
      }
    }
  }

  Some(hasher.finish())
}

fn build_file_patterns(cwd: &Path, entries: &[String]) -> Option<FilePatterns> {
  let include =
    PathOrPatternSet::from_include_relative_path_or_patterns(cwd, entries)
      .ok()?;
  Some(FilePatterns {
    base: cwd.to_path_buf(),
    include: Some(include),
    exclude: PathOrPatternSet::new(Vec::new()),
  })
}
