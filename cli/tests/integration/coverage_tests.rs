// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs;
use test_util as util;
use test_util::TempDir;
use util::TestContext;
use util::TestContextBuilder;

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
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let deno_dir = context.deno_dir();
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
  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  // Modify the file between deno test and deno coverage, thus invalidating the cache
  std::fs::copy(mod_after_path, mod_temp_path).unwrap();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .run();

  output.assert_exit_code(1);
  let out = output.combined_output();

  // Expect error
  let error = util::strip_ansi_codes(out).to_string();
  assert!(error.contains("error: Missing transpiled source code"));
  assert!(error.contains("Before generating coverage report, run `deno test --coverage` to ensure consistent state."));
}

fn run_coverage_text(test_name: &str, extension: &str) {
  let context = TestContext::default();
  let tempdir = context.deno_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "-A".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
      format!("coverage/{test_name}_test.{extension}"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .split_output()
    .run();

  // Verify there's no "Check" being printed
  assert!(output.stderr().is_empty());

  let actual = util::strip_ansi_codes(output.stdout()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join(format!("coverage/{test_name}_expected.out")),
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
    .args_vec(vec![
      "coverage".to_string(),
      "--quiet".to_string(),
      "--lcov".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .run();

  let actual = util::strip_ansi_codes(output.combined_output()).to_string();

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
  let context = TestContext::default();
  let tempdir = context.deno_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
      format!("coverage/multifile/"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .split_output()
    .run();

  // Verify there's no "Check" being printed
  assert!(output.stderr().is_empty());

  let actual = util::strip_ansi_codes(output.stdout()).to_string();

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
    .args_vec(vec![
      "coverage".to_string(),
      "--quiet".to_string(),
      "--lcov".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .run();

  let actual = util::strip_ansi_codes(output.combined_output()).to_string();

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
  let context = TestContext::default();
  let tempdir = context.deno_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--allow-read".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
      format!("coverage/no_snaps_included/{test_name}_test.{extension}"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--include=no_snaps_included.ts".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .split_output()
    .run();

  // Verify there's no "Check" being printed
  assert!(output.stderr().is_empty());

  let actual = util::strip_ansi_codes(output.stdout()).to_string();

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
fn no_npm_cache_coverage() {
  let context = TestContext::default();
  let tempdir = context.deno_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--allow-read".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
      format!("coverage/no_npm_coverage/no_npm_coverage_test.ts"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .split_output()
    .run();

  // Verify there's no "Check" being printed
  assert!(output.stderr().is_empty());

  let actual = util::strip_ansi_codes(output.stdout()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_npm_coverage/expected.out"),
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
  let context = TestContext::default();
  let tempdir = context.deno_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir.to_str().unwrap()),
      "coverage/no_transpiled_lines/".to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--include=no_transpiled_lines/index.ts".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .run();

  let actual = util::strip_ansi_codes(output.combined_output()).to_string();

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
    .args_vec(vec![
      "coverage".to_string(),
      "--lcov".to_string(),
      "--include=no_transpiled_lines/index.ts".to_string(),
      format!("{}/", tempdir.to_str().unwrap()),
    ])
    .run();

  let actual = util::strip_ansi_codes(output.combined_output()).to_string();

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
