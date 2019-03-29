// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Run "cargo build -vv" if you want to see gn output.

use std::env;
use std::path;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
  let gn_mode = if cfg!(target_os = "windows") {
    // On Windows, we need to link with a release build of libdeno, because
    // rust always uses the release CRT.
    // TODO(piscisaureus): make linking with debug libdeno possible.
    String::from("release")
  } else {
    // Cargo sets PROFILE to either "debug" or "release", which conveniently
    // matches the build modes we support.
    env::var("PROFILE").unwrap()
  };

  // Detect if we're being invoked by the rust language server (RLS).
  // Unfortunately we can't detect whether we're being run by `cargo check`.
  let check_only = env::var_os("CARGO")
    .map(PathBuf::from)
    .as_ref()
    .and_then(|p| p.file_stem())
    .and_then(|f| f.to_str())
    .map(|s| s.starts_with("rls"))
    .unwrap_or(false);

  // If we are using the same target as the host's default
  // "rustup target list" should show your default target
  let is_default_target =
    env::var("TARGET").unwrap() == env::var("HOST").unwrap();

  // Equivalent to target arch != host arch
  let is_different_target_arch =
    env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() != env::var("HOST")
      .unwrap()
      .as_str()
      .split("-")
      .collect::<Vec<&str>>()[0];

  // cd into workspace root.
  assert!(env::set_current_dir("..").is_ok());

  let cwd = env::current_dir().unwrap();
  // If not using host default target the output folder will change
  // target/release will become target/$TARGET/release
  // Gn should also be using this output directory as well
  // most things will work with gn using the default
  // output directory but some tests depend on artifacts
  // being in a specific directory relative to the main build output
  let gn_out_path = cwd.join(format!(
    "target/{}",
    match is_default_target {
      true => gn_mode.clone(),
      false => format!("{}/{}", env::var("TARGET").unwrap(), gn_mode.clone()),
    }
  ));
  let gn_out_dir = normalize_path(&gn_out_path);

  // This helps Rust source files locate the snapshot, source map etc.
  println!("cargo:rustc-env=GN_OUT_DIR={}", gn_out_dir);

  let gn_target;

  if check_only {
    // When RLS is running "cargo check" to analyze the source code, we're not
    // trying to build a working executable, rather we're just compiling all
    // rust code. Therefore, make ninja build only 'msg_generated.rs'.
    gn_target = "msg_rs";

    // Enable the 'check_only' feature, which enables some workarounds in the
    // rust source code to compile successfully without a bundle and snapshot
    println!("cargo:rustc-cfg=feature=\"check-only\"");
  } else {
    // "Full" (non-RLS) build.
    if is_different_target_arch {
      gn_target = "deno_deps_cross";
    } else {
      gn_target = "deno_deps";
    }
  }

  let status = Command::new("python")
    .env("DENO_BUILD_PATH", &gn_out_dir)
    .env("DENO_BUILD_MODE", &gn_mode)
    .arg("./tools/build.py")
    .arg(gn_target)
    .arg("-v")
    .status()
    .expect("build.py failed");
  assert!(status.success());
}

// Utility function to make a path absolute, normalizing it to use forward
// slashes only. The returned value is an owned String, otherwise panics.
fn normalize_path<T: AsRef<Path>>(path: T) -> String {
  path
    .as_ref()
    .to_str()
    .unwrap()
    .to_owned()
    .chars()
    .map(|c| if path::is_separator(c) { '/' } else { c })
    .collect()
}
