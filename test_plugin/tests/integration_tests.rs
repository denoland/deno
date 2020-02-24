// TODO(ry) Re-enable this test on windows. It is flaky for an unknown reason.
#![cfg(not(windows))]

use deno::test_util::*;
use std::process::Command;

fn deno_cmd() -> Command {
  assert!(deno_exe_path().exists());
  Command::new(deno_exe_path())
}

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
    .arg("--allow-plugin")
    .arg("tests/test_basic.js")
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
  let expected = if cfg!(target_os = "windows") {
    "Hello from plugin. data: test | zero_copy: test\nPlugin Sync Response: test\r\nHello from plugin. data: test | zero_copy: test\nPlugin Async Response: test\r\n"
  } else {
    "Hello from plugin. data: test | zero_copy: test\nPlugin Sync Response: test\nHello from plugin. data: test | zero_copy: test\nPlugin Async Response: test\n"
  };
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn dispatch_json() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_plugin");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let _build_plugin_output = build_plugin.output().unwrap();
  let output = deno_cmd()
    .arg("--allow-plugin")
    .arg("tests/test_dispatch_json.js")
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
  let expected = if cfg!(target_os = "windows") {
    "Hello from json op. size: 12 | name: testObject | zero_copy: test\nPlugin Json Response: { id: 21, name: \"testObject\" }\r\n"
  } else {
    "Hello from json op. size: 12 | name: testObject | zero_copy: test\nPlugin Json Response: { id: 21, name: \"testObject\" }\n"
  };
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
