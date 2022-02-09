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
    .arg("run")
    .arg("-A")
    .arg("strings_test.js")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();

  assert!(output.status.success());
}
