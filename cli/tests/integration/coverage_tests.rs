// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs;
use test_util as util;
use test_util::TempDir;
use util::TestContext;

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

  // Write the inital mod.ts file
  std::fs::copy(mod_before_path, &mod_temp_path).unwrap();
  // And the test file
  std::fs::copy(mod_test_path, mod_test_temp_path).unwrap();

  // Generate coverage
  let context = TestContext::default();
  let deno_dir_str = deno_dir_path.to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .cwd(deno_dir_str)
    .args(format!(
      "test --quiet --coverage={}",
      tempdir.to_str().unwrap()
    ))
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  // Modify the file between deno test and deno coverage, thus invalidating the cache
  std::fs::copy(mod_after_path, mod_temp_path).unwrap();

  let deno_dir_str = deno_dir_path.to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .cwd(deno_dir_str)
    .args(format!("coverage {}/", tempdir.to_str().unwrap()))
    .run();

  output.assert_exit_code(1);
  let out = output.text();

  // Expect error
  let error = util::strip_ansi_codes(out).to_string();
  assert!(error.contains("error: Missing transpiled source code"));
  assert!(error.contains("Before generating coverage report, run `deno test --coverage` to ensure consistent state."));
}

fn run_coverage_text(test_name: &str, extension: &str) {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let context = TestContext::default();
  let deno_dir_str = deno_dir.path().to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "test -A --quiet --coverage={} coverage/{test_name}_test.{extension}",
      tempdir.to_str().unwrap()
    ))
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

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
  // TODO: how to assert this with test builder output?
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

  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --quiet --unstable --lcov {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join(format!("coverage/{test_name}_expected.lcov")),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  output.assert_exit_code(0);
}

#[test]
fn multifile_coverage() {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let context = TestContext::default();
  let deno_dir_str = deno_dir.path().to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "test --quiet --coverage={} coverage/multifile/",
      tempdir.to_str().unwrap()
    ))
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --unstable {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  // Verify there's no "Check" being printed
  // TODO: how to assert this with test builder output?
  // assert!(output.stderr.is_empty());

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/multifile/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }
  output.assert_exit_code(0);

  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --quiet --unstable --lcov {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/multifile/expected.lcov"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  output.assert_exit_code(0);
}

fn no_snaps_included(test_name: &str, extension: &str) {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let context = TestContext::default();
  let deno_dir_str = deno_dir.path().to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "test --quiet --unstable --allow-read --coverage={} coverage/no_snaps_included/{test_name}_test.{extension}",
      tempdir.to_str().unwrap()
    ))
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --unstable --include=no_snaps_included.ts {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  // Verify there's no "Check" being printed
  // TODO
  // assert!(output.stderr.is_empty());

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_snaps_included/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  output.assert_exit_code(0);
}

#[test]
fn no_transpiled_lines() {
  let deno_dir = TempDir::new();
  let tempdir = TempDir::new();
  let tempdir = tempdir.path().join("cov");

  let context = TestContext::default();
  let deno_dir_str = deno_dir.path().to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "test --quiet --coverage={} coverage/no_transpiled_lines/",
      tempdir.to_str().unwrap()
    ))
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let deno_dir_str = deno_dir.path().to_str().unwrap();
  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --include=no_transpiled_lines/index.ts {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_transpiled_lines/expected.out"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  output.assert_exit_code(0);

  let output = context
    .new_command()
    .env("DENO_DIR", deno_dir_str)
    .args(format!(
      "coverage --lcov --include=no_transpiled_lines/index.ts {}/",
      tempdir.to_str().unwrap()
    ))
    .run();

  let actual = util::strip_ansi_codes(output.text()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_transpiled_lines/expected.lcov"),
  )
  .unwrap();

  if !util::wildcard_match(&expected, &actual) {
    println!("OUTPUT\n{actual}\nOUTPUT");
    println!("EXPECTED\n{expected}\nEXPECTED");
    panic!("pattern match failed");
  }

  output.assert_exit_code(0);
}
