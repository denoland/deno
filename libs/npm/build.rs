// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  println!("cargo:rustc-link-arg-benches=-rdynamic");
  println!("cargo:rerun-if-changed=build.rs");
}
