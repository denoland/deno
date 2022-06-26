// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

fn main() {
  // Skip building from docs.rs.
  if std::env::var_os("DOCS_RS").is_some() {
    return;
  }

  println!(
    "cargo:rustc-env=TARGET={}",
    std::env::var("TARGET").unwrap()
  );
  println!(
    "cargo:rustc-env=PROFILE={}",
    std::env::var("PROFILE").unwrap()
  );
}
