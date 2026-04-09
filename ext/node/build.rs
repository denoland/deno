// Copyright 2018-2026 the Deno authors. MIT license.

use std::env;
use std::path::PathBuf;

fn main() {
  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());

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
