// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  println!(
    "cargo:rustc-env=TARGET={}",
    std::env::var("TARGET").unwrap()
  );
}
