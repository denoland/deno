// Copyright 2018-2026 the Deno authors. MIT license.

use std::process::Command;

use test_util::deno_cmd;
use test_util::deno_config_path;
use test_util::println;
use test_util::testdata_path;
use test_util::tests_path;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn build_sqlite_extension() {
  // The extension is in a separate standalone package (excluded from workspace)
  // because it requires rusqlite's "loadable_extension" feature which is
  // incompatible with the "session" feature used by the rest of the workspace.
  let tests_dir = tests_path();
  let extension_manifest =
    tests_dir.join("sqlite_extension").join("Cargo.toml");
  // Output to the repo's target directory so the Deno tests can find it
  let target_dir = tests_dir.parent().join("target");

  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin = build_plugin_base
    .arg("build")
    .arg("--manifest-path")
    .arg(extension_manifest.as_path())
    .arg("--target-dir")
    .arg(target_dir.as_path())
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

#[test_util::test]
fn sqlite_extension() {
  build_sqlite_extension();

  let extension_test_file = testdata_path().join("sqlite_extension_test.ts");

  let output = deno_cmd()
    .arg("test")
    .arg("--allow-read")
    .arg("--allow-write")
    .arg("--allow-ffi")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-check")
    .arg(extension_test_file.as_path())
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
