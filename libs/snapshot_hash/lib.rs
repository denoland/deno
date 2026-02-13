// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;

/// Compute a hash of all snapshot inputs: file paths, file contents,
/// and version strings. This must be used identically by both the
/// snapshot generator (runtime) and the cache checker (cli/snapshot build.rs)
/// to ensure hash consistency.
pub fn compute_hash(
  files: &[&str],
  ts_version: &str,
  v8_version: &str,
  target: &str,
) -> String {
  let mut hasher = DefaultHasher::new();
  files.len().hash(&mut hasher);
  for path in files {
    path.hash(&mut hasher);
    match std::fs::read(path) {
      Ok(content) => content.hash(&mut hasher),
      Err(_) => 0u8.hash(&mut hasher),
    }
  }
  ts_version.hash(&mut hasher);
  v8_version.hash(&mut hasher);
  target.hash(&mut hasher);
  format!("{:x}", hasher.finish())
}

/// Write a manifest file listing snapshot inputs and metadata.
/// This allows downstream build scripts to recompute the snapshot hash
/// without depending on deno_runtime.
pub fn write_manifest(
  manifest_path: &Path,
  files: &[&str],
  v8_version: &str,
) {
  let mut content = String::new();
  content.push_str(&format!("v8_version={}\n", v8_version));
  content.push_str("---\n");
  for path in files {
    content.push_str(path);
    content.push('\n');
  }
  std::fs::write(manifest_path, content).unwrap();
}

/// Parse a manifest file. Returns `(v8_version, file_paths)` or `None`
/// if the format is invalid.
pub fn parse_manifest(content: &str) -> Option<(String, Vec<String>)> {
  let mut lines = content.lines();
  let v8_version = lines.next()?.strip_prefix("v8_version=")?.to_string();
  if lines.next()? != "---" {
    return None;
  }
  let paths = lines.filter(|l| !l.is_empty()).map(String::from).collect();
  Some((v8_version, paths))
}

/// Emit `cargo:rerun-if-changed` for each file path in the manifest.
#[allow(clippy::print_stdout)]
pub fn emit_rerun_from_manifest(content: &str) {
  if let Some((_, paths)) = parse_manifest(content) {
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
    let manifest = "v8_version=12.8.0\n---\n/a/b.js\n/c/d.ts\n";
    let (v8, paths) = parse_manifest(manifest).unwrap();
    assert_eq!(v8, "12.8.0");
    assert_eq!(paths, vec!["/a/b.js", "/c/d.ts"]);
  }

  #[test]
  fn test_parse_manifest_invalid() {
    assert!(parse_manifest("garbage").is_none());
    assert!(parse_manifest("v8_version=1.0\nbad").is_none());
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
