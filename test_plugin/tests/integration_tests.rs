// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// To run this test manually:
//   cd test_plugin
//   ../target/debug/deno run --unstable --allow-plugin tests/test.js debug

use std::process::Command;
use test_util::deno_cmd;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

#[test]
fn basic() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_plugin");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(build_plugin_output.status.success());
  let output = deno_cmd()
    .arg("run")
    .arg("--allow-plugin")
    .arg("--unstable")
    .arg("tests/test.js")
    .arg(BUILD_VARIANT)
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
  let expected = "Hello from plugin.\nzero_copy[0]: test\nzero_copy[1]: 123\nzero_copy[2]: cba\nPlugin Sync Response: test\nHello from plugin.\nzero_copy[0]: test\nzero_copy[1]: 123\nPlugin Async Response: test\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
