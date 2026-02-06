// Copyright 2018-2026 the Deno authors. MIT license.

fn main() {
  let target = std::env::var("TARGET").unwrap();

  // let's bind dav1d
  // The feature of "avif-native" in image relies on dav1d->dav1d-sys, it requires dav1d C library to be installed on the system
  // when it doesn't set up the toolchains to build it.
  // The additional environment variables setting of dav1d is located in .cargo/config.toml about binding.
  let dav1d_dir = match target.as_str() {
    "x86_64-unknown-linux-gnu" => "dav1d/x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu" => "dav1d/aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc" => "dav1d/x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc" => "dav1d/aarch64-pc-windows-msvc",
    "x86_64-apple-darwin" => "dav1d/x86_64-apple-darwin",
    "aarch64-apple-darwin" => "dav1d/aarch64-apple-darwin",
    _ => {
      panic!("No prebuilt dav1d library for target {}", target);
    }
  };
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

  let lib_path = std::path::Path::new(&manifest_dir).join(dav1d_dir);
  println!("cargo:rustc-link-search=native={}", lib_path.display());

  let lib_filename = if target.contains("windows") {
    "dav1d.lib"
  } else {
    "libdav1d.a"
  };
  let lib_file = lib_path.join(lib_filename);
  println!("cargo:rerun-if-changed={}", lib_file.display());
}
