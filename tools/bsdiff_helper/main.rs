// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal bsdiff CLI tool for generating binary delta patches.
//! Used by the release CI to create .bsdiff files for delta upgrades.
//!
//! Usage: bsdiff_helper <old_file> <new_file> <patch_file>

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

  let mut patch = Vec::new();
  bsdiff::diff(&old, &new, &mut patch).unwrap_or_else(|e| {
    print_stderr(&format!("Failed to generate bsdiff patch: {}", e));
    std::process::exit(1);
  });

  std::fs::write(&args[3], &patch).unwrap_or_else(|e| {
    print_stderr(&format!(
      "Failed to write patch file '{}': {}",
      args[3], e
    ));
    std::process::exit(1);
  });

  print_stderr(&format!(
    "Generated bsdiff patch: {} bytes (old: {} bytes, new: {} bytes)",
    patch.len(),
    old.len(),
    new.len()
  ));
}

fn print_stderr(msg: &str) {
  use std::io::Write;
  let _ = std::io::stderr().write_all(msg.as_bytes());
  let _ = std::io::stderr().write_all(b"\n");
}
