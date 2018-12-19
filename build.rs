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

  // Normally we configure GN+Ninja to build into Cargo's OUT_DIR.
  // However, when DENO_BUILD_PATH is set, perform the ninja build in that dir
  // instead. This is used by CI to avoid building V8 etc twice.
  let gn_out_dir = match env::var_os("DENO_BUILD_PATH") {
    None => {
      // out_dir looks like: "target/debug/build/deno-26d2b5325de0f0cf/out"
      // The gn build is "target/debug"
      // So we go up two directories. Blame cargo for these hacks.
      let d = out_dir.parent().unwrap();
      let d = d.parent().unwrap();
      let d = d.parent().unwrap();
      PathBuf::from(d)
    }
    Some(deno_build_path) => PathBuf::from(deno_build_path),
  };

  // Give cargo some instructions. We do this first so the `rerun-if-*-changed`
  // directives can take effect even if something the build itself fails.
  println!("cargo:rustc-env=GN_OUT_DIR={}", normalize_path(&gn_out_dir));
  println!(
    "cargo:rustc-link-search=native={}/obj",
    normalize_path(&gn_out_dir)
  );
  println!("cargo:rustc-link-lib=static=deno_deps");

  println!("cargo:rerun-if-env-changed=DENO_BUILD_PATH");
  // TODO: this is obviously not appropriate here.
  println!("cargo:rerun-if-env-changed=APPVEYOR_REPO_COMMIT");

  // Detect if we're being invoked by the rust language server (RLS).
  // Unfortunately we can't detect whether we're being run by `cargo check`.
  let check_only = env::var_os("CARGO")
    .map(PathBuf::from)
    .as_ref()
    .and_then(|p| p.file_stem())
    .and_then(|f| f.to_str())
    .map(|s| s.starts_with("rls"))
    .unwrap_or(false);

  // If we're being invoked by the RLS, build only the targets that are needed
  // for `cargo check` to succeed.
  let gn_target = if check_only {
    "cargo_check_deps"
  } else {
    "deno_deps"
  };

  if check_only {
    println!("cargo:rustc-cfg=feature=\"check-only\"");
  }

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
    .arg(gn_target)
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
