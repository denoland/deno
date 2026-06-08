// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal bsdiff CLI tool for generating binary delta patches.
//! Used by the release CI to create .bsdiff files for delta upgrades.
//!
//! The raw bsdiff output (control/diff/extra streams from the `bsdiff` crate)
//! is uncompressed and routinely larger than the new binary itself. We wrap
//! it in zstd here so the published `.bsdiff` artifact is actually small.
//! The client (`apply_bsdiff_patch` in `cli/tools/upgrade.rs`) sniffs the
//! zstd magic and decompresses, falling back to raw bsdiff for patches
//! produced before this change.
//!
//! Usage: bsdiff_helper <old_file> <new_file> <patch_file>

// Matches `cli/build.rs`'s zstd usage for runtime snapshots.
const ZSTD_LEVEL: i32 = 19;

fn main() {
  let args: Vec<String> = std::env::args().collect();
  if args.len() != 4 {
    print_stderr(&format!(
      "Usage: {} <old_file> <new_file> <patch_file>",
      args[0]
    ));
    std::process::exit(1);
  }

  let old = std::fs::read(&args[1]).unwrap_or_else(|e| {
    print_stderr(&format!("Failed to read old file '{}': {}", args[1], e));
    std::process::exit(1);
  });
  let new = std::fs::read(&args[2]).unwrap_or_else(|e| {
    print_stderr(&format!("Failed to read new file '{}': {}", args[2], e));
    std::process::exit(1);
  });

  let mut raw_patch = Vec::new();
  bsdiff::diff(&old, &new, &mut raw_patch).unwrap_or_else(|e| {
    print_stderr(&format!("Failed to generate bsdiff patch: {}", e));
    std::process::exit(1);
  });

  let patch =
    zstd::bulk::compress(&raw_patch, ZSTD_LEVEL).unwrap_or_else(|e| {
      print_stderr(&format!("Failed to zstd-compress bsdiff patch: {}", e));
      std::process::exit(1);
    });

  std::fs::write(&args[3], &patch).unwrap_or_else(|e| {
    print_stderr(&format!("Failed to write patch file '{}': {}", args[3], e));
    std::process::exit(1);
  });

  print_stderr(&format!(
    "Generated bsdiff patch: {} bytes compressed (raw: {} bytes, old: {} bytes, new: {} bytes)",
    patch.len(),
    raw_patch.len(),
    old.len(),
    new.len()
  ));
}

fn print_stderr(msg: &str) {
  use std::io::Write;
  let _ = std::io::stderr().write_all(msg.as_bytes());
  let _ = std::io::stderr().write_all(b"\n");
}
