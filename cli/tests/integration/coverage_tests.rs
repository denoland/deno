// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::fs;
use tempfile::TempDir;
use test_util as util;

#[test]
fn branch() {
  run_coverage_text("branch", "ts");
}

#[test]
fn complex() {
  run_coverage_text("complex", "ts");
}

#[test]
fn final_blankline() {
  run_coverage_text("final_blankline", "js");
}

fn run_coverage_text(test_name: &str, extension: &str) {
  let tempdir = TempDir::new().expect("tempdir fail");
  let tempdir = tempdir.path().join("cov");
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--quiet")
    .arg("--unstable")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .arg(format!("coverage/{}_test.{}", test_name, extension))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .status()
    .expect("failed to spawn test runner");

  assert!(status.success());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--quiet")
    .arg("--unstable")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .output()
    .expect("failed to spawn coverage reporter");

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join(format!("coverage/{}_expected.out", test_name)),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--quiet")
    .arg("--unstable")
    .arg("--lcov")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .output()
    .expect("failed to spawn coverage reporter");

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join(format!("coverage/{}_expected.lcov", test_name)),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());
}
