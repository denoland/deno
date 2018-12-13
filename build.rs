// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Run "cargo build -vv" if you want to see gn output.

#![deny(warnings)]

use std::env;
use std::path::{self, Path, PathBuf};
use std::process::Command;

fn main() {
  // Cargo sets PROFILE to either "debug" or "release", which conveniently
  // matches the build modes we support.
  let mode = env::var("PROFILE").unwrap();

  let out_dir = env::var_os("OUT_DIR").unwrap();
  let out_dir = env::current_dir().unwrap().join(out_dir);

  // Normally we configure GN+Ninja to build into Cargo's OUT_DIR. However, when
  // DENO_BUILD_PATH is set, perform the ninja build in that dir instead.
  let gn_out_dir = match env::var_os("DENO_BUILD_PATH") {
    None => {
      // out_dir looks like: "target/debug/build/deno-26d2b5325de0f0cf/out"
      // The gn build is "target/debug", so we go up three directories.
      let d = out_dir.parent().unwrap();
      let d = d.parent().unwrap();
      let d = d.parent().unwrap();
      PathBuf::from(d)
    }
    Some(deno_build_path) => PathBuf::from(deno_build_path),
  };
  let gn_out_dir = normalize_path(&gn_out_dir);

  // Tell Cargo when to re-run this file. We do this first, so these directives
  // can take effect even if something goes wrong later in the build process.
  println!("cargo:rerun-if-env-changed=DENO_BUILD_PATH");
  // TODO: this is obviously not appropriate here.
  println!("cargo:rerun-if-env-changed=APPVEYOR_REPO_COMMIT");

  // This helps Rust source files locate the snapshot, source map etc.
  println!("cargo:rustc-env=GN_OUT_DIR={}", gn_out_dir);

  // Link with libdeno, which includes V8.
  println!("cargo:rustc-link-search=native={}/obj/libdeno", gn_out_dir);
  if cfg!(target_os = "windows") {
    println!("cargo:rustc-link-lib=static=libdeno");
  } else {
    println!("cargo:rustc-link-lib=static=deno");
  }

  // Link the system libraries that libdeno and V8 depend on.
  if cfg!(any(target_os = "macos", target_os = "freebsd")) {
    println!("cargo:rustc-link-lib=dylib=c++");
  } else if cfg!(target_os = "windows") {
    for lib in vec!["dbghelp", "shlwapi", "winmm", "ws2_32"] {
      println!("cargo:rustc-link-lib={}", lib);
    }
  }

  // TODO(piscisaureus): Bring back Rust Language Server support.
  //   * Don't build any non-rust targets when the RLS is analyzing the crate.
  //   * Use a feature flag to disable the include_bytes! and include_str! macro
  //     invocations that embed the snapshot and source maps.

  let status = Command::new("python")
    .env("DENO_BUILD_PATH", &gn_out_dir)
    .env("DENO_BUILD_MODE", &mode)
    .arg("./tools/setup.py")
    .status()
    .expect("setup.py failed");
  assert!(status.success());

  let status = Command::new("python")
    .env("DENO_BUILD_PATH", &gn_out_dir)
    .env("DENO_BUILD_MODE", &mode)
    .arg("./tools/build.py")
    .arg("deno_deps")
    .arg("-v")
    .status()
    .expect("build.py failed");
  assert!(status.success());
}

// Utility function to make a path absolute, normalizing it to use forward
// slashes only. The returned value is an owned String, otherwise panics.
fn normalize_path(path: &Path) -> String {
  path
    .to_str()
    .unwrap()
    .to_owned()
    .chars()
    .map(|c| if path::is_separator(c) { '/' } else { c })
    .collect()
}
