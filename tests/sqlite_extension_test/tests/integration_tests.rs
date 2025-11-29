// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::path::Path;
use std::process::Command;

use test_util::deno_cmd;
use test_util::deno_config_path;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn build_extension() {
  // The extension is in a separate standalone package (excluded from workspace)
  // because it requires rusqlite's "loadable_extension" feature which is
  // incompatible with the "session" feature used by the rest of the workspace.
  let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
  let extension_manifest =
    tests_dir.join("sqlite_extension").join("Cargo.toml");
  // Output to the repo's target directory so the Deno tests can find it
  let target_dir = tests_dir.parent().unwrap().join("target");

  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin = build_plugin_base
    .arg("build")
    .arg("--manifest-path")
    .arg(&extension_manifest)
    .arg("--target-dir")
    .arg(&target_dir)
    // Don't inherit RUSTFLAGS from the test environment - the sysroot
    // configuration used for main Deno builds doesn't have libsqlite3
    .env_remove("RUSTFLAGS")
    .env_remove("RUSTDOCFLAGS");

  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }

  let build_plugin_output = build_plugin.output().unwrap();
  println!(
    "cargo build output: {}",
    String::from_utf8_lossy(&build_plugin_output.stdout)
  );
  println!(
    "cargo build error: {}",
    String::from_utf8_lossy(&build_plugin_output.stderr)
  );
  assert!(
    build_plugin_output.status.success(),
    "Extension build failed. Check that rusqlite features are compatible."
  );
}

#[test]
fn sqlite_extension_test() {
  build_extension();

  let extension_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let extension_test_file = extension_dir.join("sqlite_extension_test.ts");

  let output = deno_cmd()
    .arg("test")
    .arg("--allow-read")
    .arg("--allow-write")
    .arg("--allow-ffi")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-check")
    .arg(extension_test_file)
    .env("NO_COLOR", "1")
    .output()
    .unwrap();

  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  if !output.status.success() {
    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);
    panic!("Test failed");
  }
}
