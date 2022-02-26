// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
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
  assert!(out.contains("test success ... ok"));
  assert!(out.contains("test fail ... FAILED"));
  assert!(out.contains("test ignored ... ignored"));
  assert!(out.contains("test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out"));
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
  args: "test test/aggregate_error.ts",
  exit_code: 1,
  output: "test/aggregate_error.out",
});

itest!(steps_passing_steps {
  args: "test test/steps/passing_steps.ts",
  exit_code: 0,
  output: "test/steps/passing_steps.out",
});

itest!(steps_passing_steps_concurrent {
  args: "test --jobs=2 test/steps/passing_steps.ts",
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

itest!(no_prompt_by_default {
  args: "test test/no_prompt_by_default.ts",
  exit_code: 1,
  output: "test/no_prompt_by_default.out",
});

itest!(no_prompt_with_denied_perms {
  args: "test --allow-read test/no_prompt_with_denied_perms.ts",
  exit_code: 1,
  output: "test/no_prompt_with_denied_perms.out",
});
