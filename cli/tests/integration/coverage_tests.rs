// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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

#[test]
fn error_if_invalid_cache() {
  let deno_dir = TempDir::new();
  let deno_dir_path = deno_dir.path();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let invalid_cache_path = util::testdata_path().join("coverage/invalid_cache");
  let mod_before_path = util::testdata_path()
    .join(&invalid_cache_path)
    .join("mod_before.ts");
  let mod_after_path = util::testdata_path()
    .join(&invalid_cache_path)
    .join("mod_after.ts");
  let mod_test_path = util::testdata_path()
    .join(&invalid_cache_path)
    .join("mod.test.ts");

  let mod_temp_path = deno_dir_path.join("mod.ts");
  let mod_test_temp_path = deno_dir_path.join("mod.test.ts");

  // Write the initial mod.ts file
  std::fs::copy(mod_before_path, &mod_temp_path).unwrap();
  // And the test file
  std::fs::copy(mod_test_path, mod_test_temp_path).unwrap();

  // Generate coverage
  let status = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir_path)
    .arg("test")
    .arg("--quiet")
    .arg(format!("--coverage={}", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::inherit())
    .status()
    .unwrap();

  assert!(status.success());

  // Modify the file between deno test and deno coverage, thus invalidating the cache
  std::fs::copy(mod_after_path, mod_temp_path).unwrap();

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(deno_dir_path)
    .arg("coverage")
    .arg(format!("{}/", tempdir.to_str().unwrap()))
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .unwrap();

  assert!(output.stdout.is_empty());

  // Expect error
  let error =
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap())
      .to_string();
  assert!(error.contains("error: Missing transpiled source code"));
  assert!(error.contains("Before generating coverage report, run `deno test --coverage` to ensure consistent state."));
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
    .arg(format!("coverage/{test_name}_test.{extension}"))
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
    util::testdata_path().join(format!("coverage/{test_name}_expected.out")),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
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
    util::testdata_path().join(format!("coverage/{test_name}_expected.lcov")),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
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
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
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
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
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
      "coverage/no_snaps_included/{test_name}_test.{extension}"
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
    .arg("--include=no_snaps_included.ts")
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
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
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
    .arg("--include=no_transpiled_lines/index.ts")
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
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  assert!(output.status.success());

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("coverage")
    .arg("--lcov")
    .arg("--include=no_transpiled_lines/index.ts")
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
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  assert!(output.status.success());
}
