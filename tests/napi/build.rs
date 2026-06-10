// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  // On macOS, native modules need undefined symbols to be resolved
  // at runtime by the host process.
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
    println!("cargo:rustc-cdylib-link-arg=-Wl");
    println!("cargo:rustc-cdylib-link-arg=-undefined");
    println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
  }
}
