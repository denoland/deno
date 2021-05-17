// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
  println!("{:?}", output.status);
  assert!(output.status.success());
  let expected = "\
    Plugin rid: 3\n\
    Hello from sync plugin op.\n\
    args: TestArgs { val: \"1\" }\n\
    zero_copy: test\n\
    op_test_sync returned: test\n\
    Hello from async plugin op.\n\
    args: TestArgs { val: \"1\" }\n\
    zero_copy: 123\n\
    op_test_async returned: test\n\
    Hello from resource_table.add plugin op.\n\
    TestResource rid: 4\n\
    Hello from resource_table.get plugin op.\n\
    TestResource get value: hello plugin!\n\
    Hello from sync plugin op.\n\
    args: TestArgs { val: \"1\" }\n\
    Ops completed count is correct!\n\
    Ops dispatched count is correct!\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
