// Copyright 2018-2026 the Deno authors. MIT license.

use std::env;

fn main() {
  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
}
