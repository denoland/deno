// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use deno_core::url::Url;
use test_util as util;

#[test]
fn no_color() {
  let (out, _) = util::run_and_collect_output(
    false,
    "test test/no_color.ts",
    None,
    Some(vec![("NO_COLOR".to_owned(), "true".to_owned())]),
    false,
  );
  // ANSI escape codes should be stripped.
  assert!(out.contains("success ... ok"));
  assert!(out.contains("fail ... FAILED"));
  assert!(out.contains("ignored ... ignored"));
  assert!(out.contains("FAILED | 1 passed | 1 failed | 1 ignored"));
}

itest!(overloads {
  args: "test test/overloads.ts",
  exit_code: 0,
  output: "test/overloads.out",
});

itest!(meta {
  args: "test test/meta.ts",
  exit_code: 0,
  output: "test/meta.out",
});

itest!(pass {
  args: "test test/pass.ts",
  exit_code: 0,
  output: "test/pass.out",
});

itest!(ignore {
  args: "test test/ignore.ts",
  exit_code: 0,
  output: "test/ignore.out",
});

itest!(ignore_permissions {
  args: "test --unstable test/ignore_permissions.ts",
  exit_code: 0,
  output: "test/ignore_permissions.out",
});

itest!(fail {
  args: "test test/fail.ts",
  exit_code: 1,
  output: "test/fail.out",
});

itest!(collect {
  args: "test --ignore=test/collect/ignore test/collect",
  exit_code: 0,
  output: "test/collect.out",
});

itest!(test_with_config {
  args: "test --config test/collect/deno.jsonc test/collect",
  exit_code: 0,
  output: "test/collect.out",
});

itest!(test_with_config2 {
  args: "test --config test/collect/deno2.jsonc test/collect",
  exit_code: 0,
  output: "test/collect2.out",
});

itest!(test_with_malformed_config {
  args: "test --config test/collect/deno.malformed.jsonc",
  exit_code: 1,
  output: "test/collect_with_malformed_config.out",
});

itest!(parallel_flag {
  args: "test test/short-pass.ts --parallel",
  exit_code: 0,
  output: "test/short-pass.out",
});

itest!(parallel_flag_with_env_variable {
  args: "test test/short-pass.ts --parallel",
  envs: vec![("DENO_JOBS".to_owned(), "2".to_owned())],
  exit_code: 0,
  output: "test/short-pass.out",
});

itest!(jobs_flag {
  args: "test test/short-pass.ts --jobs",
  exit_code: 0,
  output: "test/short-pass-jobs-flag-warning.out",
});

itest!(jobs_flag_with_numeric_value {
  args: "test test/short-pass.ts --jobs=2",
  exit_code: 0,
  output: "test/short-pass-jobs-flag-warning.out",
});

itest!(load_unload {
  args: "test test/load_unload.ts",
  exit_code: 0,
  output: "test/load_unload.out",
});

itest!(interval {
  args: "test test/interval.ts",
  exit_code: 0,
  output: "test/interval.out",
});

itest!(doc {
  args: "test --doc --allow-all test/doc.ts",
  exit_code: 1,
  output: "test/doc.out",
});

itest!(doc_only {
  args: "test --doc --allow-all test/doc_only",
  exit_code: 0,
  output: "test/doc_only.out",
});

itest!(markdown {
  args: "test --doc --allow-all test/markdown.md",
  exit_code: 1,
  output: "test/markdown.out",
});

itest!(markdown_windows {
  args: "test --doc --allow-all test/markdown_windows.md",
  exit_code: 1,
  output: "test/markdown_windows.out",
});

itest!(markdown_full_block_names {
  args: "test --doc --allow-all test/markdown_full_block_names.md",
  exit_code: 1,
  output: "test/markdown_full_block_names.out",
});

itest!(markdown_ignore_html_comment {
  args: "test --doc --allow-all test/markdown_with_comment.md",
  exit_code: 1,
  output: "test/markdown_with_comment.out",
});

itest!(text {
  args: "test --doc --allow-all test/text.md",
  exit_code: 0,
  output: "test/text.out",
});

itest!(quiet {
  args: "test --quiet test/quiet.ts",
  exit_code: 0,
  output: "test/quiet.out",
});

itest!(fail_fast {
  args: "test --fail-fast test/fail_fast.ts",
  exit_code: 1,
  output: "test/fail_fast.out",
});

itest!(only {
  args: "test test/only.ts",
  exit_code: 1,
  output: "test/only.out",
});

itest!(no_check {
  args: "test --no-check test/no_check.ts",
  exit_code: 1,
  output: "test/no_check.out",
});

itest!(no_run {
  args: "test --unstable --no-run test/no_run.ts",
  output: "test/no_run.out",
  exit_code: 1,
});

itest!(allow_all {
  args: "test --unstable --allow-all test/allow_all.ts",
  exit_code: 0,
  output: "test/allow_all.out",
});

itest!(allow_none {
  args: "test --unstable test/allow_none.ts",
  exit_code: 1,
  output: "test/allow_none.out",
});

itest!(ops_sanitizer_unstable {
  args: "test --unstable --trace-ops test/ops_sanitizer_unstable.ts",
  exit_code: 1,
  output: "test/ops_sanitizer_unstable.out",
});

itest!(ops_sanitizer_timeout_failure {
  args: "test test/ops_sanitizer_timeout_failure.ts",
  output: "test/ops_sanitizer_timeout_failure.out",
});

itest!(ops_sanitizer_multiple_timeout_tests {
  args: "test --trace-ops test/ops_sanitizer_multiple_timeout_tests.ts",
  exit_code: 1,
  output: "test/ops_sanitizer_multiple_timeout_tests.out",
});

itest!(ops_sanitizer_multiple_timeout_tests_no_trace {
  args: "test test/ops_sanitizer_multiple_timeout_tests.ts",
  exit_code: 1,
  output: "test/ops_sanitizer_multiple_timeout_tests_no_trace.out",
});

// TODO(@littledivy): re-enable this test, recent optimizations made output non deterministic.
// https://github.com/denoland/deno/issues/14268
//
// itest!(ops_sanitizer_missing_details {
//  args: "test --allow-write --allow-read test/ops_sanitizer_missing_details.ts",
//  exit_code: 1,
//  output: "test/ops_sanitizer_missing_details.out",
// });

itest!(ops_sanitizer_nexttick {
  args: "test test/ops_sanitizer_nexttick.ts",
  output: "test/ops_sanitizer_nexttick.out",
});

itest!(resource_sanitizer {
  args: "test --allow-read test/resource_sanitizer.ts",
  exit_code: 1,
  output: "test/resource_sanitizer.out",
});

itest!(exit_sanitizer {
  args: "test test/exit_sanitizer.ts",
  output: "test/exit_sanitizer.out",
  exit_code: 1,
});

itest!(clear_timeout {
  args: "test test/clear_timeout.ts",
  exit_code: 0,
  output: "test/clear_timeout.out",
});

itest!(finally_timeout {
  args: "test test/finally_timeout.ts",
  exit_code: 1,
  output: "test/finally_timeout.out",
});

itest!(unresolved_promise {
  args: "test test/unresolved_promise.ts",
  exit_code: 1,
  output: "test/unresolved_promise.out",
});

itest!(unhandled_rejection {
  args: "test test/unhandled_rejection.ts",
  exit_code: 1,
  output: "test/unhandled_rejection.out",
});

itest!(filter {
  args: "test --filter=foo test/filter",
  exit_code: 0,
  output: "test/filter.out",
});

itest!(shuffle {
  args: "test --shuffle test/shuffle",
  exit_code: 0,
  output_str: Some("[WILDCARD]"),
});

itest!(shuffle_with_seed {
  args: "test --shuffle=42 test/shuffle",
  exit_code: 0,
  output: "test/shuffle.out",
});

itest!(aggregate_error {
  args: "test --quiet test/aggregate_error.ts",
  exit_code: 1,
  output: "test/aggregate_error.out",
});

itest!(steps_passing_steps {
  args: "test test/steps/passing_steps.ts",
  exit_code: 0,
  output: "test/steps/passing_steps.out",
});

itest!(steps_failing_steps {
  args: "test test/steps/failing_steps.ts",
  exit_code: 1,
  output: "test/steps/failing_steps.out",
});

itest!(steps_ignored_steps {
  args: "test test/steps/ignored_steps.ts",
  exit_code: 0,
  output: "test/steps/ignored_steps.out",
});

itest!(steps_invalid_usage {
  args: "test test/steps/invalid_usage.ts",
  exit_code: 1,
  output: "test/steps/invalid_usage.out",
});

itest!(steps_output_within {
  args: "test test/steps/output_within.ts",
  exit_code: 0,
  output: "test/steps/output_within.out",
});

itest!(no_prompt_by_default {
  args: "test --quiet test/no_prompt_by_default.ts",
  exit_code: 1,
  output: "test/no_prompt_by_default.out",
});

itest!(no_prompt_with_denied_perms {
  args: "test --quiet --allow-read test/no_prompt_with_denied_perms.ts",
  exit_code: 1,
  output: "test/no_prompt_with_denied_perms.out",
});

itest!(test_with_custom_jsx {
  args: "test --quiet --allow-read test/hello_world.ts --config=test/deno_custom_jsx.json",
  exit_code: 0,
  output: "test/hello_world.out",
});

#[test]
fn captured_output() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--allow-run")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("test/captured_output.ts")
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  let output_start = "------- output -------";
  let output_end = "----- output end -----";
  assert!(output.status.success());
  let output_text = String::from_utf8(output.stdout).unwrap();
  let start = output_text.find(output_start).unwrap() + output_start.len();
  let end = output_text.find(output_end).unwrap();
  // replace zero width space that may appear in test output due
  // to test runner output flusher
  let output_text = output_text[start..end]
    .replace('\u{200B}', "")
    .trim()
    .to_string();
  let mut lines = output_text.lines().collect::<Vec<_>>();
  // the output is racy on either stdout or stderr being flushed
  // from the runtime into the rust code, so sort it... the main
  // thing here to ensure is that we're capturing the output in
  // this block on stdout
  lines.sort_unstable();
  assert_eq!(lines.join(" "), "0 1 2 3 4 5 6 7 8 9");
}

#[test]
fn recursive_permissions_pledge() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("test/recursive_permissions_pledge.js")
    .stderr(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8(output.stderr).unwrap().contains(
    "pledge test permissions called before restoring previous pledge"
  ));
}

#[test]
fn file_protocol() {
  let file_url =
    Url::from_file_path(util::testdata_path().join("test/file_protocol.ts"))
      .unwrap()
      .to_string();

  (util::CheckOutputIntegrationTest {
    args_vec: vec!["test", &file_url],
    exit_code: 0,
    output: "test/file_protocol.out",
    ..Default::default()
  })
  .run();
}

itest!(uncaught_errors {
  args: "test --quiet test/uncaught_errors_1.ts test/uncaught_errors_2.ts test/uncaught_errors_3.ts",
  output: "test/uncaught_errors.out",
  exit_code: 1,
});

itest!(check_local_by_default {
  args: "test --quiet test/check_local_by_default.ts",
  output: "test/check_local_by_default.out",
  http_server: true,
});

itest!(check_local_by_default2 {
  args: "test --quiet test/check_local_by_default2.ts",
  output: "test/check_local_by_default2.out",
  http_server: true,
  exit_code: 1,
});

itest!(non_error_thrown {
  args: "test --quiet test/non_error_thrown.ts",
  output: "test/non_error_thrown.out",
  exit_code: 1,
});

itest!(parallel_output {
  args: "test --parallel --reload test/parallel_output.ts",
  output: "test/parallel_output.out",
  exit_code: 1,
});
