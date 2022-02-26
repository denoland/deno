// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
    .arg("-A")
    .arg("--ignore=third_party_tests/")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(output.status.success());
}

#[test]
fn thrid_party_tests() {
  build();

  let output = deno_cmd()
    .current_dir(test_util::napi_tests_path().join("third_party_tests/"))
    .arg("test")
    .arg("--compat")
    .arg("-A")
    .arg("--unstable")
    .arg("--no-check")
    .arg("--ignore=node_modules/")
    .env("DENO_NODE_COMPAT_URL", "file:///Users/divy/gh/deno_std/")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(output.status.success());
}
