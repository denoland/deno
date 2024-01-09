// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use std::fs;
use test_util as util;
use test_util::TempDir;
use util::assert_starts_with;
use util::env_vars_for_npm_tests;
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

// TODO(mmastrac): The exclusion to make this test pass doesn't seem to work on windows.
#[cfg_attr(windows, ignore)]
#[test]
fn no_tests() {
  no_tests_included("foo", "mts");
  no_tests_included("foo", "ts");
  no_tests_included("foo", "js");
}

#[test]
fn error_if_invalid_cache() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir_path = context.temp_dir().path();
  let other_temp_dir = TempDir::new();
  let other_tempdir = other_temp_dir.path().join("cov");

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

  let mod_temp_path = temp_dir_path.join("mod.ts");
  let mod_test_temp_path = temp_dir_path.join("mod.test.ts");

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
      format!("--coverage={}", other_tempdir),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  // Modify the file between deno test and deno coverage, thus invalidating the cache
  std::fs::copy(mod_after_path, mod_temp_path).unwrap();

  let output = context
    .new_command()
    .args_vec(vec!["coverage".to_string(), format!("{}/", other_tempdir)])
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
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "-A".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
      format!("coverage/{test_name}_test.{extension}"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--detailed".to_string(),
      format!("{}/", tempdir),
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
      format!("{}/", tempdir),
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
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
      format!("coverage/multifile/"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--detailed".to_string(),
      format!("{}/", tempdir),
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
      format!("{}/", tempdir),
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
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--allow-read".to_string(),
      format!("--coverage={}", tempdir),
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
      "--detailed".to_string(),
      format!("{}/", tempdir),
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

fn no_tests_included(test_name: &str, extension: &str) {
  let context = TestContext::default();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--allow-read".to_string(),
      format!("--coverage={}", tempdir),
      format!("coverage/no_tests_included/{test_name}.test.{extension}"),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      format!("--exclude={}", util::std_path().canonicalize()),
      "--detailed".to_string(),
      format!("{}/", tempdir),
    ])
    .split_output()
    .run();

  // Verify there's no "Check" being printed
  assert!(output.stderr().is_empty());

  let actual = util::strip_ansi_codes(output.stdout()).to_string();

  let expected = fs::read_to_string(
    util::testdata_path().join("coverage/no_tests_included/expected.out"),
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
  let context = TestContext::with_http_server();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--allow-read".to_string(),
      format!("--coverage={}", tempdir),
      format!("coverage/no_npm_coverage/no_npm_coverage_test.ts"),
    ])
    .envs(env_vars_for_npm_tests())
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--detailed".to_string(),
      format!("{}/", tempdir),
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
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
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
      "--detailed".to_string(),
      format!("{}/", tempdir),
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
      format!("{}/", tempdir),
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

#[test]
fn no_internal_code() {
  let context = TestContext::default();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
      "coverage/no_internal_code_test.ts".to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  // Check that coverage files contain no internal urls
  let paths = fs::read_dir(tempdir).unwrap();
  for path in paths {
    let unwrapped = path.unwrap().path();
    let data = fs::read_to_string(&unwrapped.clone()).unwrap();

    let value: serde_json::Value = serde_json::from_str(&data).unwrap();
    let url = value["url"].as_str().unwrap();
    assert_starts_with!(url, "file:");
  }
}

#[test]
fn no_internal_node_code() {
  let context = TestContext::default();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      "--no-check".to_string(),
      format!("--coverage={}", tempdir),
      "coverage/no_internal_node_code_test.ts".to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  // Check that coverage files contain no internal urls
  let paths = fs::read_dir(tempdir).unwrap();
  for path in paths {
    let unwrapped = path.unwrap().path();
    let data = fs::read_to_string(&unwrapped.clone()).unwrap();

    let value: serde_json::Value = serde_json::from_str(&data).unwrap();
    let url = value["url"].as_str().unwrap();
    assert_starts_with!(url, "file:");
  }
}

#[test]
fn test_html_reporter() {
  let context = TestContext::default();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
      "coverage/multisource".to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec![
      "coverage".to_string(),
      "--html".to_string(),
      format!("{}/", tempdir),
    ])
    .run();

  output.assert_exit_code(0);
  output.assert_matches_text("HTML coverage report has been generated at [WILDCARD]/cov/html/index.html\n");

  let index_html =
    fs::read_to_string(tempdir.join("html").join("index.html")).unwrap();
  assert!(index_html.contains("<h1>Coverage report for all files</h1>"));
  assert!(index_html.contains("baz/"));
  assert!(index_html.contains("href='baz/index.html'"));
  assert!(index_html.contains("foo.ts"));
  assert!(index_html.contains("href='foo.ts.html'"));
  assert!(index_html.contains("bar.ts"));
  assert!(index_html.contains("href='bar.ts.html'"));

  let foo_ts_html =
    fs::read_to_string(tempdir.join("html").join("foo.ts.html")).unwrap();
  assert!(foo_ts_html.contains("<h1>Coverage report for foo.ts</h1>"));

  let bar_ts_html =
    fs::read_to_string(tempdir.join("html").join("bar.ts.html")).unwrap();
  assert!(bar_ts_html.contains("<h1>Coverage report for bar.ts</h1>"));
  // Check <T> in source code is escaped to &lt;T&gt;
  assert!(bar_ts_html.contains("&lt;T&gt;"));

  let baz_index_html =
    fs::read_to_string(tempdir.join("html").join("baz").join("index.html"))
      .unwrap();
  assert!(baz_index_html.contains("<h1>Coverage report for baz/</h1>"));
  assert!(baz_index_html.contains("qux.ts"));
  assert!(baz_index_html.contains("href='qux.ts.html'"));
  assert!(baz_index_html.contains("quux.ts"));
  assert!(baz_index_html.contains("href='quux.ts.html'"));

  let baz_qux_ts_html =
    fs::read_to_string(tempdir.join("html").join("baz").join("qux.ts.html"))
      .unwrap();
  assert!(baz_qux_ts_html.contains("<h1>Coverage report for baz/qux.ts</h1>"));

  let baz_quux_ts_html =
    fs::read_to_string(tempdir.join("html").join("baz").join("quux.ts.html"))
      .unwrap();
  assert!(baz_quux_ts_html.contains("<h1>Coverage report for baz/quux.ts</h1>"));
}

#[test]
fn test_summary_reporter() {
  let context = TestContext::default();
  let tempdir = context.temp_dir();
  let tempdir = tempdir.path().join("cov");

  let output = context
    .new_command()
    .args_vec(vec![
      "test".to_string(),
      "--quiet".to_string(),
      format!("--coverage={}", tempdir),
      "coverage/multisource".to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .args_vec(vec!["coverage".to_string(), format!("{}/", tempdir)])
    .run();

  output.assert_exit_code(0);
  output.assert_matches_text(
    "----------------------------------
File         | Branch % | Line % |
----------------------------------
 bar.ts      |      0.0 |   57.1 |
 baz/quux.ts |      0.0 |   28.6 |
 baz/qux.ts  |    100.0 |  100.0 |
 foo.ts      |     50.0 |   76.9 |
----------------------------------
 All files   |     40.0 |   61.0 |
----------------------------------
",
  );
}

itest!(no_files_found {
  args: "coverage doesnt_exist.js",
  exit_code: 1,
  output: "coverage/doesnt_exist.out",
});
