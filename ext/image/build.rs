// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  let target = std::env::var("TARGET").unwrap();

  // let's bind dav1d
  let dav1d_dir = match target.as_str() {
    "x86_64-unknown-linux-gnu" => "dav1d/x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu" => "dav1d/aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc" => "dav1d/x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc" => "dav1d/aarch64-pc-windows-msvc",
    "x86_64-apple-darwin" => "dav1d/x86_64-apple-darwin",
    "aarch64-apple-darwin" => "dav1d/aarch64-apple-darwin",
    _ => {
      eprintln!("Warning: No prebuilt dav1d library for target {}", target);
      return;
    }
  };
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let lib_path = std::path::Path::new(&manifest_dir).join(dav1d_dir);
  println!("cargo:rustc-link-search=native={}", lib_path.display());
  println!("cargo:rerun-if-changed={}", lib_path.display());
}
