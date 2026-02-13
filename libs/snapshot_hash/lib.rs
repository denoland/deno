// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hasher;
use std::path::Path;
use twox_hash::XxHash64;

const MANIFEST_VERSION: u32 = 1;

/// Compute a hash of all snapshot inputs: file paths, file contents,
/// and version strings. This must be used identically by both the
/// snapshot generator (runtime) and the cache checker (cli/snapshot build.rs)
/// to ensure hash consistency.
///
/// Uses XxHash64 for stable, cross-toolchain-version hashing.
/// Fields are delimited explicitly to avoid ambiguity.
pub fn compute_hash(
  files: &[&str],
  ts_version: &str,
  v8_version: &str,
  target: &str,
) -> String {
  let mut hasher = XxHash64::with_seed(0);
  // Hash the file count as a fixed-width value
  hasher.write(&(files.len() as u64).to_le_bytes());
  for path in files {
    // Length-prefix each string field to avoid ambiguity
    hasher.write(&(path.len() as u64).to_le_bytes());
    hasher.write(path.as_bytes());
    match std::fs::read(path) {
      Ok(content) => {
        hasher.write(&(content.len() as u64).to_le_bytes());
        hasher.write(&content);
      }
      Err(_) => {
        // Sentinel for missing file
        hasher.write(&u64::MAX.to_le_bytes());
      }
    }
  }
  hasher.write(&(ts_version.len() as u64).to_le_bytes());
  hasher.write(ts_version.as_bytes());
  hasher.write(&(v8_version.len() as u64).to_le_bytes());
  hasher.write(v8_version.as_bytes());
  hasher.write(&(target.len() as u64).to_le_bytes());
  hasher.write(target.as_bytes());
  format!("{:x}", hasher.finish())
}

/// Write a manifest file listing snapshot inputs and metadata.
/// This allows downstream build scripts to recompute the snapshot hash
/// without depending on deno_runtime.
pub fn write_manifest(
  manifest_path: &Path,
  files: &[&str],
  ts_version: &str,
  v8_version: &str,
) {
  let mut content = String::new();
  content.push_str(&format!("manifest_version={}\n", MANIFEST_VERSION));
  content.push_str(&format!("ts_version={}\n", ts_version));
  content.push_str(&format!("v8_version={}\n", v8_version));
  content.push_str("---\n");
  for path in files {
    content.push_str(path);
    content.push('\n');
  }
  std::fs::write(manifest_path, content).unwrap();
}

/// Parse a manifest file. Returns `(ts_version, v8_version, file_paths)` or
/// `None` if the format is invalid or the version is unsupported.
pub fn parse_manifest(content: &str) -> Option<(String, String, Vec<String>)> {
  let mut lines = content.lines();
  let version_str = lines.next()?.strip_prefix("manifest_version=")?;
  let version: u32 = version_str.parse().ok()?;
  if version != MANIFEST_VERSION {
    // Unknown version — trigger a cache miss
    return None;
  }
  let ts_version = lines.next()?.strip_prefix("ts_version=")?.to_string();
  let v8_version = lines.next()?.strip_prefix("v8_version=")?.to_string();
  if lines.next()? != "---" {
    return None;
  }
  let paths = lines.filter(|l| !l.is_empty()).map(String::from).collect();
  Some((ts_version, v8_version, paths))
}

/// Emit `cargo:rerun-if-changed` for each file path in the manifest.
#[allow(clippy::print_stdout)]
pub fn emit_rerun_from_manifest(content: &str) {
  if let Some((_, _, paths)) = parse_manifest(content) {
    for path in &paths {
      println!("cargo:rerun-if-changed={}", path);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_manifest_roundtrip() {
    let manifest =
      "manifest_version=1\nts_version=5.9.2\nv8_version=12.8.0\n---\n/a/b.js\n/c/d.ts\n";
    let (ts, v8, paths) = parse_manifest(manifest).unwrap();
    assert_eq!(ts, "5.9.2");
    assert_eq!(v8, "12.8.0");
    assert_eq!(paths, vec!["/a/b.js", "/c/d.ts"]);
  }

  #[test]
  fn test_parse_manifest_invalid() {
    assert!(parse_manifest("garbage").is_none());
    assert!(parse_manifest("manifest_version=1\nts_version=1.0\nbad").is_none());
  }

  #[test]
  fn test_parse_manifest_unknown_version() {
    let manifest =
      "manifest_version=99\nts_version=5.9.2\nv8_version=12.8.0\n---\n/a.js\n";
    assert!(parse_manifest(manifest).is_none());
  }

  #[test]
  fn test_compute_hash_deterministic() {
    // With non-existent files, the hash should still be deterministic
    let h1 = compute_hash(
      &["/nonexistent/a.js"],
      "5.9.2",
      "12.8.0",
      "x86_64-unknown-linux-gnu",
    );
    let h2 = compute_hash(
      &["/nonexistent/a.js"],
      "5.9.2",
      "12.8.0",
      "x86_64-unknown-linux-gnu",
    );
    assert_eq!(h1, h2);
  }

  #[test]
  fn test_compute_hash_varies_with_input() {
    let h1 = compute_hash(&[], "5.9.2", "12.8.0", "x86_64-unknown-linux-gnu");
    let h2 = compute_hash(&[], "5.9.3", "12.8.0", "x86_64-unknown-linux-gnu");
    assert_ne!(h1, h2);
  }
}
