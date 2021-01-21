// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// To run this test manually:
//   cargo test ffi_tests

use std::{io::Read, process::Command};
use test_util::deno_cmd;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

#[test]
fn ffi_tests() {
  let mut build_dylib_base = Command::new("cargo");
  let mut build_dylib = build_dylib_base.arg("build").arg("-p").arg("test_ffi");
  if BUILD_VARIANT == "release" {
    build_dylib = build_dylib.arg("--release");
  }
  let build_dylib_output = build_dylib.output().unwrap();
  assert!(build_dylib_output.status.success());
  let output = deno_cmd()
    .arg("run")
    .arg("--allow-all")
    .arg("--unstable")
    .arg("tests/test.js")
    .arg(BUILD_VARIANT)
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
  let mut file = std::fs::File::open("tests/test.out").unwrap();
  let mut expected = String::new();
  file.read_to_string(&mut expected).unwrap();
  println!("{}", stdout);
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
