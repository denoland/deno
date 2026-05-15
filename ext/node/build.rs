// Copyright 2018-2026 the Deno authors. MIT license.

use std::env;
use std::path::PathBuf;

fn main() {
  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());

  // Polyfills are embedded at compile time via include_str! through
  // the deno_core extension! macro. Cargo doesn't auto-track those,
  // so a `cargo build` after editing a polyfill misses the change.
  // Walk polyfills/ and js/ trees, emit rerun-if-changed for each.
  for root in &["polyfills", "js"] {
    let path = PathBuf::from(root);
    if !path.exists() {
      continue;
    }
    walk_and_emit(&path);
  }

  // Compile llhttp (Node.js HTTP/1.1 parser)
  let llhttp_dir = PathBuf::from("ops/llhttp/c");
  cc::Build::new()
    .files([
      llhttp_dir.join("llhttp.c"),
      llhttp_dir.join("http.c"),
      llhttp_dir.join("api.c"),
    ])
    .include(&llhttp_dir)
    .std("c99")
    .warnings(false)
    .compile("llhttp");
}

fn walk_and_emit(path: &std::path::Path) {
  if path.is_dir() {
    if let Ok(entries) = std::fs::read_dir(path) {
      for e in entries.flatten() {
        walk_and_emit(&e.path());
      }
    }
  } else {
    println!("cargo:rerun-if-changed={}", path.display());
  }
}
