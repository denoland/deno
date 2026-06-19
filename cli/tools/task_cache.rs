// Copyright 2018-2026 the Deno authors. MIT license.

//! Input-based caching for `deno task`.
//!
//! When a task declares `files`, Deno computes a fingerprint over the
//! contents of those files, the command string (including any arguments
//! appended on the CLI), the values of any environment variables listed in
//! `env`, and the fingerprints of the task's dependencies. If a previous run
//! wrote a matching fingerprint to disk, the task is skipped and any declared
//! `output` artifacts are restored from the cache.
//!
//! The fingerprint is split in two so the common "nothing changed" case
//! avoids reading file contents at all:
//!
//! - a *static* hash over everything that isn't a file (command, appended
//!   argv, listed env values, dependency fingerprints, and the
//!   platform/version salt), and
//! - a *content* hash over the input file set (relative paths + contents).
//!
//! Each entry also records a per-input stat snapshot (size + mtime). On
//! lookup, if the static hash and every input's size+mtime match the stored
//! manifest, the contents cannot have changed and we hit without reading
//! them. Only when a stat drifts do we fall back to rehashing contents,
//! which also tells a real edit from an mtime-only touch (e.g. a `git`
//! checkout that rewrote timestamps).
//!
//! Both hashes are SHA-256: a user-facing build cache must not risk a
//! collision silently skipping a build with stale output, so we use a wide
//! cryptographic hash rather than a fast non-cryptographic one.
//!
//! Outputs are captured into the cache directory after a successful run and
//! restored on a hit, so deleting a task's declared outputs (e.g. removing
//! `dist/`) and re-running regenerates them from the cache instead of
//! silently leaving them missing. Stale outputs from a previous run are
//! cleaned before a re-run so a build does not mix old and new artifacts.
//!
//! The on-disk format is a small JSON manifest per task plus a directory of
//! captured outputs; an unrecognized or older payload simply fails to parse
//! and is treated as a miss, so the schema can evolve without migration.
//!
//! Caveat: the fingerprint is captured before the run and stored afterwards,
//! so a task that writes into its own `files` (formatters, codegen) changes
//! its inputs as a side effect and will never match on the next run. It stays
//! correct, just never caches.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::serde_json;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::sys::CliSys;

/// Result of consulting the cache for a task.
pub enum CacheLookup {
  /// The task is eligible for caching and the stored fingerprint matches, so
  /// the task may be skipped. Carries the task's fingerprint so dependents can
  /// fold it into their own cache key.
  Hit(String),
  /// The task is eligible for caching but no stored fingerprint matches.
  /// Caller should run the task and then call [`TaskCache::store`].
  Miss(Fingerprint),
  /// The task does not opt in to caching (no `files` declared), or one of its
  /// dependencies is itself non-cacheable and so always runs.
  NotCacheable,
}

/// A freshly computed fingerprint, ready to be persisted after a successful
/// run.
pub struct Fingerprint {
  /// The task's overall fingerprint, exposed so the runner can record it for
  /// dependent tasks' cascade keys.
  pub fingerprint: String,
  static_hash: String,
  content_hash: String,
  files: Vec<FileStat>,
  /// Outputs captured by the previous successful run, if any. Removed before
  /// the re-run so stale artifacts don't linger.
  prior_outputs: Vec<String>,
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

  /// Compute the fingerprint for a task and check whether it matches what's on
  /// disk. On a hit, declared outputs are restored from the cache.
  pub fn lookup(&self, key: &TaskCacheKey<'_>) -> CacheLookup {
    if key.files.is_empty() {
      return CacheLookup::NotCacheable;
    }
    // A dependency that always runs (non-cacheable) makes this task unsafe to
    // cache: it could produce new artifacts on every run that this task
    // consumes. Propagate non-cacheability downstream.
    let Some(dep_fingerprints) = key.dep_fingerprints else {
      return CacheLookup::NotCacheable;
    };
    let Some(inputs) = collect_inputs(key) else {
      return CacheLookup::NotCacheable;
    };
    let static_hash = compute_static_hash(key, dep_fingerprints);
    let stats: Vec<FileStat> = inputs.iter().map(|i| i.stat.clone()).collect();

    let stored = std::fs::read_to_string(self.manifest_path(key))
      .ok()
      .and_then(|s| serde_json::from_str::<CacheManifest>(&s).ok());

    // Fast path: the static inputs and every file's size+mtime match the
    // stored manifest, so the contents cannot have changed. Skip reading them.
    if let Some(stored) = &stored
      && stored.static_hash == static_hash
      && stored.files == stats
    {
      self.restore_outputs(key, stored);
      return CacheLookup::Hit(stored.fingerprint.clone());
    }

    // Slow path: something drifted. Read contents to tell a real edit from an
    // mtime-only touch.
    let content_hash = compute_content_hash(key, &inputs);
    let fingerprint = combine_fingerprint(&static_hash, &content_hash);

    if let Some(stored) = &stored
      && stored.static_hash == static_hash
      && stored.content_hash == content_hash
    {
      // Contents are identical; only stat metadata moved. Refresh the manifest
      // so the next run takes the fast path again, restore outputs, and skip.
      let manifest = CacheManifest {
        fingerprint: fingerprint.clone(),
        static_hash,
        content_hash,
        files: stats,
        outputs: stored.outputs.clone(),
      };
      self.write_manifest(key, &manifest);
      self.restore_outputs(key, stored);
      return CacheLookup::Hit(fingerprint);
    }

    CacheLookup::Miss(Fingerprint {
      fingerprint,
      static_hash,
      content_hash,
      files: stats,
      prior_outputs: stored.map(|s| s.outputs).unwrap_or_default(),
    })
  }

  /// Remove the outputs captured by a previous run before re-running, so a
  /// fresh build does not mix stale and new artifacts. Only files this task
  /// produced itself (recorded in the previous manifest) are removed.
  pub fn clean_stale_outputs(
    &self,
    key: &TaskCacheKey<'_>,
    fingerprint: &Fingerprint,
  ) {
    for rel in &fingerprint.prior_outputs {
      let path = key.cwd.join(rel);
      if let Err(err) = remove_if_exists(&path) {
        log::debug!("failed to remove stale output {}: {err}", path.display());
      }
    }
  }

  /// Persist a fingerprint and capture the task's declared outputs for a
  /// successfully completed run.
  pub fn store(&self, key: &TaskCacheKey<'_>, fingerprint: &Fingerprint) {
    let outputs = self.capture_outputs(key);
    let manifest = CacheManifest {
      fingerprint: fingerprint.fingerprint.clone(),
      static_hash: fingerprint.static_hash.clone(),
      content_hash: fingerprint.content_hash.clone(),
      files: fingerprint.files.clone(),
      outputs,
    };
    self.write_manifest(key, &manifest);
  }

  /// Copy the files matching the task's `output` globs into the cache so they
  /// can be restored on a later hit. Returns the captured relative paths.
  fn capture_outputs(&self, key: &TaskCacheKey<'_>) -> Vec<String> {
    if key.output.is_empty() {
      return Vec::new();
    }
    let Some(files) = collect_files(key.cwd, key.output) else {
      return Vec::new();
    };
    let outputs_dir = self.outputs_dir(key);
    // Start from a clean slate so removed artifacts don't linger in the cache.
    let _ = remove_if_exists(&outputs_dir);
    let mut captured = Vec::with_capacity(files.len());
    for abs in files {
      let Ok(rel) = abs.strip_prefix(key.cwd) else {
        continue;
      };
      let dest = outputs_dir.join(rel);
      if let Some(parent) = dest.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
      {
        log::debug!("failed to create output cache dir: {err}");
        continue;
      }
      if let Err(err) = std::fs::copy(&abs, &dest) {
        log::debug!("failed to cache output {}: {err}", abs.display());
        continue;
      }
      captured.push(rel.to_string_lossy().into_owned());
    }
    captured
  }

  /// Restore captured outputs to the working tree. Files that are missing or
  /// whose contents differ from the cached copy are rewritten; identical files
  /// are left untouched so we don't churn mtimes needlessly.
  fn restore_outputs(&self, key: &TaskCacheKey<'_>, manifest: &CacheManifest) {
    let outputs_dir = self.outputs_dir(key);
    for rel in &manifest.outputs {
      let cached = outputs_dir.join(rel);
      let dest = key.cwd.join(rel);
      if !cached.exists() {
        continue;
      }
      if files_have_equal_contents(&cached, &dest) {
        continue;
      }
      if let Some(parent) = dest.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
      {
        log::debug!("failed to create output dir {}: {err}", parent.display());
        continue;
      }
      if let Err(err) = std::fs::copy(&cached, &dest) {
        log::debug!("failed to restore output {}: {err}", dest.display());
      }
    }
  }

  fn write_manifest(&self, key: &TaskCacheKey<'_>, manifest: &CacheManifest) {
    let entry_dir = self.entry_dir(key);
    if let Err(err) = std::fs::create_dir_all(&entry_dir) {
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
    let path = self.manifest_path(key);
    if let Err(err) = std::fs::write(&path, json) {
      log::debug!("failed to write task cache entry {}: {err}", path.display());
    }
  }

  fn entry_dir(&self, key: &TaskCacheKey<'_>) -> PathBuf {
    // Hash the identity (package + task name + cwd) so we don't have to worry
    // about path-unsafe characters and so workspace members with identical
    // task names don't collide. The directory name is deliberately *not*
    // version-sensitive: a Deno upgrade should reuse (and overwrite) the same
    // entry, not orphan it under a new name and leak the old one. Version
    // sensitivity lives in the fingerprint contents instead, so an upgrade
    // simply produces a miss and the entry is rewritten in place.
    let mut hasher = Sha256::new();
    hasher.update(b"deno-task-cache-identity-v1");
    hasher.update(key.package_name.unwrap_or("").as_bytes());
    hasher.update([0]);
    hasher.update(key.task_name.as_bytes());
    hasher.update([0]);
    hasher.update(key.cwd.as_os_str().as_encoded_bytes());
    self.dir.join(faster_hex::hex_string(&hasher.finalize()))
  }

  fn manifest_path(&self, key: &TaskCacheKey<'_>) -> PathBuf {
    self.entry_dir(key).join("manifest.json")
  }

  fn outputs_dir(&self, key: &TaskCacheKey<'_>) -> PathBuf {
    self.entry_dir(key).join("outputs")
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
  /// Globs identifying output artifacts. Captured after a successful run and
  /// restored on a hit.
  pub output: &'a [String],
  pub env_names: &'a [String],
  /// Values of the environment variables named in `env_names`, snapshotted at
  /// fingerprint time. Only the listed names are read, so this need not carry
  /// the rest of the environment.
  pub env: &'a BTreeMap<String, String>,
  /// Fingerprints of the task's direct dependencies, folded into the static
  /// hash so a downstream task re-runs when an upstream one did. `None` means
  /// at least one dependency is non-cacheable (always runs), which makes this
  /// task non-cacheable too.
  pub dep_fingerprints: Option<&'a [String]>,
}

/// On-disk cache entry for a single task.
#[derive(Serialize, Deserialize)]
struct CacheManifest {
  /// Overall fingerprint, `hash(static_hash, content_hash)`. Used as the
  /// dependency-cascade key for downstream tasks.
  fingerprint: String,
  /// Hash of everything that isn't an input file: the command, appended argv,
  /// listed env values, dependency fingerprints, and the platform/version
  /// salt.
  static_hash: String,
  /// Hash of the input file set (relative paths + contents).
  content_hash: String,
  /// Per-input stat snapshot, enabling the size+mtime fast path that skips
  /// re-reading contents when nothing has changed.
  files: Vec<FileStat>,
  /// Relative paths of the outputs captured under `outputs/`, for restoration
  /// on a hit and cleanup before a re-run.
  #[serde(default)]
  outputs: Vec<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct FileStat {
  /// Path relative to the task's cwd.
  path: String,
  size: u64,
  /// Nanoseconds since the Unix epoch; 0 when the platform or filesystem does
  /// not report a modification time.
  mtime: u64,
}

/// A matching input file together with its stat snapshot.
struct InputFile {
  abs_path: PathBuf,
  stat: FileStat,
}

/// Resolve the input globs, returning the matching files (sorted) and their
/// stat snapshots. Returns `None` when caching does not apply: either the
/// patterns are invalid or the globs match nothing (almost certainly a config
/// error, which would otherwise silently make the cache always hit).
fn collect_inputs(key: &TaskCacheKey<'_>) -> Option<Vec<InputFile>> {
  let files = collect_files(key.cwd, key.files)?;
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

/// Resolve a set of globs relative to `cwd` into a sorted list of matching
/// files. Returns `None` for invalid patterns or when nothing matches.
fn collect_files(cwd: &Path, globs: &[String]) -> Option<Vec<PathBuf>> {
  let patterns = build_file_patterns(cwd, globs)?;
  let mut files = FileCollector::new(|_| true)
    .ignore_git_folder()
    .ignore_node_modules()
    .collect_file_patterns(&CliSys::default(), &patterns);
  files.sort();
  if files.is_empty() {
    return None;
  }
  Some(files)
}

/// Hash everything that isn't an input file's contents: the command, the
/// appended CLI args, the listed env values, the dependency fingerprints, and
/// a platform/version salt.
fn compute_static_hash(
  key: &TaskCacheKey<'_>,
  dep_fingerprints: &[String],
) -> String {
  let mut hasher = FingerprintHasher::new("deno-task-cache-static-v1");
  // Fold in the target OS, architecture, and Deno version: a task's output can
  // legitimately differ across platforms or releases even when its inputs are
  // byte-for-byte identical, so a cache shared across them (e.g. a synced
  // DENO_DIR) must not hit across them.
  hasher.write_str(env!("CARGO_PKG_VERSION"));
  hasher.write_str(std::env::consts::OS);
  hasher.write_str(std::env::consts::ARCH);
  hasher.write_str(key.command);

  // Appended CLI args materially change what runs, so fold them in.
  hasher.write_u64(key.argv.len() as u64);
  for arg in key.argv {
    hasher.write_str(arg);
  }

  // Dependency fingerprints, sorted for determinism. A change upstream
  // (an input edit that made a dependency re-run) bubbles down here.
  let mut deps = dep_fingerprints.to_vec();
  deps.sort();
  hasher.write_u64(deps.len() as u64);
  for dep in &deps {
    hasher.write_str(dep);
  }

  // Capture only the env vars the user explicitly named, in a deterministic
  // order. Missing vars hash with a distinguishing tag.
  let mut sorted_env = key.env_names.to_vec();
  sorted_env.sort();
  sorted_env.dedup();
  for name in &sorted_env {
    hasher.write_str(name);
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

  hasher.finish_hex()
}

/// Hash the input file set: each file's relative path mixed with its contents,
/// in the sorted order produced by [`collect_inputs`].
fn compute_content_hash(
  key: &TaskCacheKey<'_>,
  inputs: &[InputFile],
) -> String {
  let mut hasher = FingerprintHasher::new("deno-task-cache-content-v1");
  for input in inputs {
    let rel = input
      .abs_path
      .strip_prefix(key.cwd)
      .unwrap_or(&input.abs_path);
    hasher.write_bytes(rel.as_os_str().as_encoded_bytes());
    match std::fs::read(&input.abs_path) {
      Ok(bytes) => {
        hasher.write_u8(1);
        hasher.write_bytes(&bytes);
      }
      Err(_) => {
        hasher.write_u8(0);
      }
    }
  }
  hasher.finish_hex()
}

/// Combine the static and content hashes into the task's overall fingerprint.
fn combine_fingerprint(static_hash: &str, content_hash: &str) -> String {
  let mut hasher = FingerprintHasher::new("deno-task-cache-combined-v1");
  hasher.write_str(static_hash);
  hasher.write_str(content_hash);
  hasher.finish_hex()
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

fn remove_if_exists(path: &Path) -> std::io::Result<()> {
  match std::fs::metadata(path) {
    Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
    Ok(_) => std::fs::remove_file(path),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(err) => Err(err),
  }
}

/// Whether two files exist and have byte-identical contents. Used to avoid
/// rewriting an output that already matches the cached copy.
fn files_have_equal_contents(a: &Path, b: &Path) -> bool {
  match (std::fs::read(a), std::fs::read(b)) {
    (Ok(a), Ok(b)) => a == b,
    _ => false,
  }
}

/// A SHA-256 hasher that length-prefixes every field so distinct sequences of
/// writes can never collide by concatenation.
struct FingerprintHasher(Sha256);

impl FingerprintHasher {
  fn new(domain: &str) -> Self {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    Self(hasher)
  }

  fn write_bytes(&mut self, bytes: &[u8]) {
    self.0.update((bytes.len() as u64).to_le_bytes());
    self.0.update(bytes);
  }

  fn write_str(&mut self, s: &str) {
    self.write_bytes(s.as_bytes());
  }

  fn write_u8(&mut self, value: u8) {
    self.0.update([value]);
  }

  fn write_u64(&mut self, value: u64) {
    self.0.update(value.to_le_bytes());
  }

  fn finish_hex(self) -> String {
    faster_hex::hex_string(&self.0.finalize())
  }
}
