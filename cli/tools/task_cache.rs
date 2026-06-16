// Copyright 2018-2026 the Deno authors. MIT license.

//! Input-based caching for `deno task`.
//!
//! When a task declares `files`, Deno computes a fingerprint over the
//! contents of those files, the command string (including any arguments
//! appended on the CLI), and the values of any environment variables
//! listed in `env`. If a previous run wrote a matching fingerprint to
//! disk, the task is skipped.
//!
//! The fingerprint is split in two so the common "nothing changed" case
//! avoids reading file contents at all:
//!
//! - a *static* hash over everything that isn't a file (command, appended
//!   argv, listed env values, and the platform/version salt), and
//! - a *content* hash over the input file set (relative paths + contents).
//!
//! Each entry also records a per-input stat snapshot (size + mtime). On
//! lookup, if the static hash and every input's size+mtime match the stored
//! manifest, the contents cannot have changed and we hit without reading
//! them. Only when a stat drifts do we fall back to rehashing contents,
//! which also tells a real edit from an mtime-only touch (e.g. a `git`
//! checkout that rewrote timestamps).
//!
//! This is the first pass: no output restoration, no dependency
//! fingerprint propagation, no remote cache. The on-disk format is a small
//! JSON manifest per task; an unrecognized or older payload simply fails to
//! parse and is treated as a miss, so the schema can evolve without
//! migration.
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
use std::time::UNIX_EPOCH;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::serde_json;
use deno_lib::util::hash::FastInsecureHasher;
use serde::Deserialize;
use serde::Serialize;

use crate::sys::CliSys;

/// Result of consulting the cache for a task.
pub enum CacheLookup {
  /// The task is eligible for caching and the stored fingerprint matches,
  /// so the task may be skipped.
  Hit,
  /// The task is eligible for caching but no stored fingerprint matches.
  /// Caller should run the task and then call [`TaskCache::store`] with
  /// the returned fingerprint.
  Miss(Fingerprint),
  /// The task does not opt in to caching (no `files` declared).
  NotCacheable,
}

/// A freshly computed fingerprint plus the input stat snapshot it was
/// derived from, ready to be persisted after a successful run.
pub struct Fingerprint {
  manifest: CacheManifest,
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
    let Some(inputs) = collect_inputs(key) else {
      return CacheLookup::NotCacheable;
    };
    let static_hash = compute_static_hash(key);
    let stats: Vec<FileStat> = inputs.iter().map(|i| i.stat.clone()).collect();

    let stored = std::fs::read_to_string(self.entry_path(key))
      .ok()
      .and_then(|s| serde_json::from_str::<CacheManifest>(&s).ok());

    // Fast path: the static inputs and every file's size+mtime match the
    // stored manifest, so the contents cannot have changed. Skip reading
    // them.
    if let Some(stored) = &stored
      && stored.static_hash == static_hash
      && stored.files == stats
    {
      return CacheLookup::Hit;
    }

    // Slow path: something drifted. Read contents to tell a real edit from
    // a mtime-only touch.
    let content_hash = compute_content_hash(key, &inputs);
    let manifest = CacheManifest {
      static_hash,
      content_hash,
      files: stats,
    };

    if let Some(stored) = &stored
      && stored.static_hash == static_hash
      && stored.content_hash == content_hash
    {
      // Contents are identical; only stat metadata moved. Refresh the
      // manifest so the next run takes the fast path again, and skip the
      // task.
      self.write_manifest(key, &manifest);
      return CacheLookup::Hit;
    }

    CacheLookup::Miss(Fingerprint { manifest })
  }

  /// Persist a fingerprint for a successfully completed task.
  pub fn store(&self, key: &TaskCacheKey<'_>, fingerprint: &Fingerprint) {
    self.write_manifest(key, &fingerprint.manifest);
  }

  fn write_manifest(&self, key: &TaskCacheKey<'_>, manifest: &CacheManifest) {
    if let Err(err) = std::fs::create_dir_all(&self.dir) {
      log::debug!("failed to create task cache dir: {err}");
      return;
    }
    let json = match serde_json::to_string(manifest) {
      Ok(json) => json,
      Err(err) => {
        log::debug!("failed to serialize task cache entry: {err}");
        return;
      }
    };
    let path = self.entry_path(key);
    if let Err(err) = std::fs::write(&path, json) {
      log::debug!("failed to write task cache entry {}: {err}", path.display());
    }
  }

  fn entry_path(&self, key: &TaskCacheKey<'_>) -> PathBuf {
    // Hash the identity (package + task name + cwd) so we don't have to
    // worry about path-unsafe characters and so workspace members with
    // identical task names don't collide. The filename is deliberately
    // *not* version-sensitive: a Deno upgrade should reuse (and overwrite)
    // the same entry, not orphan it under a new name and leak the old one.
    // Version sensitivity lives in the fingerprint contents instead, so an
    // upgrade simply produces a miss and the entry is rewritten in place.
    let mut hasher = FastInsecureHasher::new_without_deno_version();
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

/// On-disk cache entry for a single task.
#[derive(Serialize, Deserialize)]
struct CacheManifest {
  /// Hash of everything that isn't an input file: the command, appended
  /// argv, listed env values, and the platform/version salt.
  static_hash: u64,
  /// Hash of the input file set (relative paths + contents).
  content_hash: u64,
  /// Per-input stat snapshot, enabling the size+mtime fast path that skips
  /// re-reading contents when nothing has changed.
  files: Vec<FileStat>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct FileStat {
  /// Path relative to the task's cwd.
  path: String,
  size: u64,
  /// Nanoseconds since the Unix epoch; 0 when the platform or filesystem
  /// does not report a modification time.
  mtime: u64,
}

/// A matching input file together with its stat snapshot.
struct InputFile {
  abs_path: PathBuf,
  stat: FileStat,
}

/// Resolve the input globs, returning the matching files (sorted) and their
/// stat snapshots. Returns `None` when caching does not apply: either the
/// patterns are invalid or the globs match nothing (almost certainly a
/// config error, which would otherwise silently make the cache always hit).
fn collect_inputs(key: &TaskCacheKey<'_>) -> Option<Vec<InputFile>> {
  let patterns = build_file_patterns(key.cwd, key.files)?;
  let mut files = FileCollector::new(|_| true)
    .ignore_git_folder()
    .ignore_node_modules()
    .collect_file_patterns(&CliSys::default(), &patterns);
  files.sort();
  if files.is_empty() {
    return None;
  }
  Some(
    files
      .into_iter()
      .map(|abs_path| {
        let rel = abs_path
          .strip_prefix(key.cwd)
          .unwrap_or(&abs_path)
          .to_string_lossy()
          .into_owned();
        let (size, mtime) = match std::fs::metadata(&abs_path) {
          Ok(meta) => {
            let mtime = meta
              .modified()
              .ok()
              .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
              .map(|d| d.as_nanos() as u64)
              .unwrap_or(0);
            (meta.len(), mtime)
          }
          Err(_) => (0, 0),
        };
        InputFile {
          abs_path,
          stat: FileStat {
            path: rel,
            size,
            mtime,
          },
        }
      })
      .collect(),
  )
}

/// Hash everything that isn't an input file's contents: the command, the
/// appended CLI args, the listed env values, and a platform/version salt.
fn compute_static_hash(key: &TaskCacheKey<'_>) -> u64 {
  // `new_deno_versioned` folds the Deno version in, so an upgrade
  // invalidates every entry. Mix in the target OS and architecture too: a
  // task's output can legitimately differ across platforms even when its
  // inputs are byte-for-byte identical, so a cache shared across targets
  // (e.g. a synced DENO_DIR) must not hit across them.
  let mut hasher = FastInsecureHasher::new_deno_versioned();
  hasher.write_str("deno-task-cache-static-v1");
  hasher.write_str(std::env::consts::OS);
  hasher.write_u8(0);
  hasher.write_str(std::env::consts::ARCH);
  hasher.write_u8(0);
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

  hasher.finish()
}

/// Hash the input file set: each file's relative path mixed with its
/// contents, in the sorted order produced by [`collect_inputs`].
fn compute_content_hash(key: &TaskCacheKey<'_>, inputs: &[InputFile]) -> u64 {
  let mut hasher = FastInsecureHasher::new_deno_versioned();
  hasher.write_str("deno-task-cache-content-v1");
  for input in inputs {
    let rel = input
      .abs_path
      .strip_prefix(key.cwd)
      .unwrap_or(&input.abs_path);
    hasher.write(rel.as_os_str().as_encoded_bytes());
    hasher.write_u8(0);
    match std::fs::read(&input.abs_path) {
      Ok(bytes) => {
        hasher.write_u8(1);
        hasher.write(&bytes);
      }
      Err(_) => {
        hasher.write_u8(0);
      }
    }
  }
  hasher.finish()
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
