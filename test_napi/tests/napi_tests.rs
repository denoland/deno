// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::process::Command;
use test_util::deno_cmd;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn build() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_napi");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(build_plugin_output.status.success());
}

#[test]
fn napi_tests() {
  build();

  let output = deno_cmd()
    .current_dir(test_util::napi_tests_path())
    .arg("test")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-ffi")
    .arg("--allow-run")
    .arg("--unstable")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
}
