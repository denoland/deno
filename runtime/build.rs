// Copyright 2018-2026 the Deno authors. MIT license.
//
// Track the JS files under js/ and shared.rs sources so cargo
// re-runs include_str! whenever they change.

use std::path::Path;

fn main() {
  println!("cargo:rerun-if-changed=build.rs");
  walk(Path::new("js"));
}

fn walk(p: &Path) {
  if p.is_dir() {
    if let Ok(entries) = std::fs::read_dir(p) {
      for e in entries.flatten() {
        walk(&e.path());
      }
    }
  } else if p.is_file() {
    println!("cargo:rerun-if-changed={}", p.display());
  }
}
