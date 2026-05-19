// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use crate::colors;
use crate::util::display;

/// Parses a human-readable size string into bytes.
///
/// Supports suffixes: B, K/KB, M/MB, G/GB, T/TB (case-insensitive).
/// No suffix is treated as bytes.
///
/// Examples: "2G", "500MB", "1024K", "100", "1.5GB"
pub fn parse_cache_max_size(value: &str) -> Result<u64, AnyError> {
  let value = value.trim();
  if value.is_empty() {
    bail!("empty value for cache max size");
  }

  let value_upper = value.to_ascii_uppercase();

  let (num_str, multiplier) = if let Some(n) = value_upper.strip_suffix("TB") {
    (n, 1u64 << 40)
  } else if let Some(n) = value_upper.strip_suffix("GB") {
    (n, 1u64 << 30)
  } else if let Some(n) = value_upper.strip_suffix("MB") {
    (n, 1u64 << 20)
  } else if let Some(n) = value_upper.strip_suffix("KB") {
    (n, 1u64 << 10)
  } else if let Some(n) = value_upper.strip_suffix('T') {
    (n, 1u64 << 40)
  } else if let Some(n) = value_upper.strip_suffix('G') {
    (n, 1u64 << 30)
  } else if let Some(n) = value_upper.strip_suffix('M') {
    (n, 1u64 << 20)
  } else if let Some(n) = value_upper.strip_suffix('K') {
    (n, 1u64 << 10)
  } else if let Some(n) = value_upper.strip_suffix('B') {
    (n, 1u64)
  } else {
    (value_upper.as_str(), 1u64)
  };

  let num: f64 = num_str.parse().map_err(|_| {
    deno_core::anyhow::anyhow!("invalid cache max size: {}", value)
  })?;

  if num < 0.0 {
    bail!("cache max size must be non-negative: {}", value);
  }

  Ok((num * multiplier as f64) as u64)
}

struct FileEntry {
  path: std::path::PathBuf,
  size: u64,
  mtime: std::time::SystemTime,
}

/// Checks `DENO_CACHE_MAX_SIZE` env var and trims the cache directory
/// if it exceeds the limit, removing oldest files first.
pub fn maybe_trim_cache(deno_dir_root: &Path) {
  let max_size = match std::env::var("DENO_CACHE_MAX_SIZE") {
    Ok(val) => match parse_cache_max_size(&val) {
      Ok(size) => size,
      Err(err) => {
        log::warn!(
          "{} Invalid DENO_CACHE_MAX_SIZE value '{}': {}",
          colors::yellow("Warning"),
          val,
          err
        );
        return;
      }
    },
    Err(_) => return,
  };

  if max_size == 0 {
    return;
  }

  if !deno_dir_root.exists() {
    return;
  }

  // Collect all files with their sizes and modification times.
  let mut files = Vec::new();
  let mut total_size: u64 = 0;

  for entry in walkdir::WalkDir::new(deno_dir_root)
    .into_iter()
    .filter_map(|e| e.ok())
  {
    if !entry.file_type().is_file() {
      continue;
    }
    let meta = match entry.metadata() {
      Ok(m) => m,
      Err(_) => continue,
    };
    let size = meta.len();
    let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    total_size += size;
    files.push(FileEntry {
      path: entry.into_path(),
      size,
      mtime,
    });
  }

  if total_size <= max_size {
    return;
  }

  // Sort by mtime ascending (oldest first) so we remove stale files first.
  files.sort_by(|a, b| a.mtime.cmp(&b.mtime));

  let mut bytes_removed: u64 = 0;
  let mut files_removed: u64 = 0;

  for file in &files {
    if total_size <= max_size {
      break;
    }
    // Skip tiny metadata files that are important.
    let file_name = file
      .path
      .file_name()
      .map(|n| n.to_string_lossy())
      .unwrap_or_default();
    if file_name == "latest.txt" || file_name == "deno_history.txt" {
      continue;
    }
    if std::fs::remove_file(&file.path).is_ok() {
      total_size -= file.size;
      bytes_removed += file.size;
      files_removed += 1;
    }
  }

  // Clean up empty directories (bottom-up).
  for entry in walkdir::WalkDir::new(deno_dir_root)
    .contents_first(true)
    .into_iter()
    .filter_map(|e| e.ok())
  {
    if entry.file_type().is_dir() && entry.path() != deno_dir_root {
      // remove_dir only succeeds if the directory is empty
      let _ = std::fs::remove_dir(entry.path());
    }
  }

  if files_removed > 0 {
    log::info!(
      "{} {} (DENO_CACHE_MAX_SIZE={})",
      colors::green("Cache trimmed"),
      colors::gray(&format!(
        "{} files, {}",
        files_removed,
        display::human_size(bytes_removed as f64)
      )),
      display::human_size(max_size as f64)
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_cache_max_size_bytes() {
    assert_eq!(parse_cache_max_size("100").unwrap(), 100);
    assert_eq!(parse_cache_max_size("0").unwrap(), 0);
    assert_eq!(parse_cache_max_size("100B").unwrap(), 100);
    assert_eq!(parse_cache_max_size("100b").unwrap(), 100);
  }

  #[test]
  fn test_parse_cache_max_size_kilobytes() {
    assert_eq!(parse_cache_max_size("1K").unwrap(), 1024);
    assert_eq!(parse_cache_max_size("1KB").unwrap(), 1024);
    assert_eq!(parse_cache_max_size("1kb").unwrap(), 1024);
    assert_eq!(parse_cache_max_size("1024K").unwrap(), 1024 * 1024);
  }

  #[test]
  fn test_parse_cache_max_size_megabytes() {
    assert_eq!(parse_cache_max_size("1M").unwrap(), 1024 * 1024);
    assert_eq!(parse_cache_max_size("1MB").unwrap(), 1024 * 1024);
    assert_eq!(parse_cache_max_size("500M").unwrap(), 500 * 1024 * 1024);
  }

  #[test]
  fn test_parse_cache_max_size_gigabytes() {
    assert_eq!(parse_cache_max_size("1G").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("1GB").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("2G").unwrap(), 2 * 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("2gb").unwrap(), 2 * 1024 * 1024 * 1024);
  }

  #[test]
  fn test_parse_cache_max_size_terabytes() {
    assert_eq!(
      parse_cache_max_size("1T").unwrap(),
      1024u64 * 1024 * 1024 * 1024
    );
    assert_eq!(
      parse_cache_max_size("1TB").unwrap(),
      1024u64 * 1024 * 1024 * 1024
    );
  }

  #[test]
  fn test_parse_cache_max_size_fractional() {
    assert_eq!(
      parse_cache_max_size("1.5G").unwrap(),
      (1.5 * 1024.0 * 1024.0 * 1024.0) as u64
    );
    assert_eq!(
      parse_cache_max_size("0.5MB").unwrap(),
      (0.5 * 1024.0 * 1024.0) as u64
    );
  }

  #[test]
  fn test_parse_cache_max_size_case_insensitive() {
    assert_eq!(parse_cache_max_size("2g").unwrap(), 2 * 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("2G").unwrap(), 2 * 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("2Gb").unwrap(), 2 * 1024 * 1024 * 1024);
    assert_eq!(parse_cache_max_size("2GB").unwrap(), 2 * 1024 * 1024 * 1024);
  }

  #[test]
  fn test_parse_cache_max_size_whitespace() {
    assert_eq!(
      parse_cache_max_size(" 2G ").unwrap(),
      2 * 1024 * 1024 * 1024
    );
  }

  #[test]
  fn test_parse_cache_max_size_invalid() {
    assert!(parse_cache_max_size("").is_err());
    assert!(parse_cache_max_size("abc").is_err());
    assert!(parse_cache_max_size("G").is_err());
    assert!(parse_cache_max_size("-1G").is_err());
  }
}
