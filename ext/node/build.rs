// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::env;

fn main() {
  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
}
