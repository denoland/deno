// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fs;
use test_util as util;
use test_util::TempDir;

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

#[test]
fn no_snaps() {
  no_snaps_included("no_snaps_included", "ts");
}

fn run_coverage_text(test_name: &str, extension: &str) {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("-A")
    .arg("--quiet")
    .arg("--unstable")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .arg(format!("coverage/{}_test.{}", test_name, extension))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .status()
    .unwrap();

  assert!(status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--unstable")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .unwrap();

  // Verify there's no "Check" being printed
  assert!(output.stderr.is_empty());

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

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--quiet")
    .arg("--unstable")
    .arg("--lcov")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .output()
    .unwrap();

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

#[test]
fn multifile_coverage() {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--quiet")
    .arg("--unstable")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .arg("coverage/multifile/")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .status()
    .unwrap();

  assert!(status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--unstable")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .unwrap();

  // Verify there's no "Check" being printed
  assert!(output.stderr.is_empty());

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/multifile/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--quiet")
    .arg("--unstable")
    .arg("--lcov")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .output()
    .unwrap();

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/multifile/expected.lcov"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());
}

fn no_snaps_included(test_name: &str, extension: &str) {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--quiet")
    .arg("--unstable")
    .arg("--allow-read")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .arg(format!(
      "coverage/no_snaps_included/{}_test.{}",
      test_name, extension
    ))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .status()
    .unwrap();

  assert!(status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--unstable")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .unwrap();

  // Verify there's no "Check" being printed
  assert!(output.stderr.is_empty());

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_snaps_included/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());
}

#[test]
fn no_transpiled_lines() {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--quiet")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .arg("coverage/no_transpiled_lines/")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .status()
    .unwrap();

  assert!(status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .unwrap();

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_transpiled_lines/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--lcov")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .output()
    .unwrap();

  let actual =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_transpiled_lines/expected.lcov"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{}\nOUTPUT", actual);
    println!("EXPECTED\n{}\nEXPECTED", expected);
    panic!("pattern match failed");
  }

  assert!(output.status.success());
}
