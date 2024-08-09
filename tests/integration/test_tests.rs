// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use deno_core::url::Url;
use test_util as util;
use test_util::itest;
use util::assert_contains;
use util::assert_not_contains;
use util::env_vars_for_npm_tests;
use util::wildcard_match;
use util::TestContext;
use util::TestContextBuilder;

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
  args: "test test/ignore_permissions.ts",
  exit_code: 0,
  output: "test/ignore_permissions.out",
});

itest!(fail {
  args: "test test/fail.ts",
  exit_code: 1,
  output: "test/fail.out",
});

// GHA CI seems to have a problem with Emoji
// https://github.com/denoland/deno/pull/23200#issuecomment-2134032695
#[test]
fn fail_with_contain_unicode_filename() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "fail_with_contain_unicode_filename🦕.ts",
    "Deno.test(\"test 0\", () => {
  throw new Error();
});
    ",
  );
  let output = context
    .new_command()
    .args("test fail_with_contain_unicode_filename🦕.ts")
    .run();
  output.skip_output_check();
  output.assert_exit_code(1);
  output.assert_matches_text(
    "Check [WILDCARD]/fail_with_contain_unicode_filename🦕.ts
running 1 test from ./fail_with_contain_unicode_filename🦕.ts
test 0 ... FAILED ([WILDCARD])

 ERRORS 

test 0 => ./fail_with_contain_unicode_filename🦕.ts:[WILDCARD]
error: Error
  throw new Error();
        ^
    at [WILDCARD]/fail_with_contain_unicode_filename🦕.ts:[WILDCARD]

 FAILURES 

test 0 => ./fail_with_contain_unicode_filename🦕.ts:[WILDCARD]

FAILED | 0 passed | 1 failed ([WILDCARD])

error: Test failed
",
  );
}

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

itest!(test_with_deprecated_config {
  args: "test --config test/collect/deno.deprecated.jsonc test/collect",
  exit_code: 0,
  output: "test/collect.deprecated.out",
});

itest!(test_with_malformed_config {
  args: "test --config test/collect/deno.malformed.jsonc",
  exit_code: 1,
  output: "test/collect_with_malformed_config.out",
});

itest!(test_filtered_out_only {
  args: "test --quiet --filter foo test/filtered_out_only.ts",
  output: "test/filtered_out_only.out",
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
  args: "test --fail-fast test/fail_fast.ts test/fail_fast_other.ts",
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
  args: "test --no-run test/no_run.ts",
  output: "test/no_run.out",
  exit_code: 1,
});

itest!(allow_all {
  args: "test --config ../config/deno.json --allow-all test/allow_all.ts",
  exit_code: 0,
  output: "test/allow_all.out",
});

itest!(allow_none {
  args: "test --config ../config/deno.json test/allow_none.ts",
  exit_code: 1,
  output: "test/allow_none.out",
});

itest!(ops_sanitizer_unstable {
  args: "test --trace-ops test/sanitizer/ops_sanitizer_unstable.ts",
  exit_code: 1,
  output: "test/sanitizer/ops_sanitizer_unstable.out",
});

itest!(ops_sanitizer_timeout_failure {
  args: "test test/sanitizer/ops_sanitizer_timeout_failure.ts",
  output: "test/sanitizer/ops_sanitizer_timeout_failure.out",
});

itest!(ops_sanitizer_multiple_timeout_tests {
  args:
    "test --trace-leaks test/sanitizer/ops_sanitizer_multiple_timeout_tests.ts",
  exit_code: 1,
  output: "test/sanitizer/ops_sanitizer_multiple_timeout_tests.out",
});

itest!(ops_sanitizer_multiple_timeout_tests_no_trace {
  args: "test test/sanitizer/ops_sanitizer_multiple_timeout_tests.ts",
  exit_code: 1,
  output: "test/sanitizer/ops_sanitizer_multiple_timeout_tests_no_trace.out",
});

itest!(sanitizer_trace_ops_catch_error {
  args: "test -A --trace-leaks test/sanitizer/trace_ops_caught_error/main.ts",
  exit_code: 0,
  output: "test/sanitizer/trace_ops_caught_error/main.out",
});

itest!(ops_sanitizer_closed_inside_started_before {
  args: "test --trace-leaks test/sanitizer/ops_sanitizer_closed_inside_started_before.ts",
  exit_code: 1,
  output: "test/sanitizer/ops_sanitizer_closed_inside_started_before.out",
});

itest!(ops_sanitizer_nexttick {
  args: "test --no-check test/sanitizer/ops_sanitizer_nexttick.ts",
  output: "test/sanitizer/ops_sanitizer_nexttick.out",
});

itest!(resource_sanitizer {
  args: "test --allow-read test/sanitizer/resource_sanitizer.ts",
  exit_code: 1,
  output: "test/sanitizer/resource_sanitizer.out",
});

itest!(ops_sanitizer_tcp {
  args: "test --allow-net --trace-leaks test/sanitizer/ops_sanitizer_tcp.ts",
  exit_code: 1,
  output: "test/sanitizer/ops_sanitizer_tcp.out",
});

itest!(exit_sanitizer {
  args: "test test/sanitizer/exit_sanitizer.ts",
  output: "test/sanitizer/exit_sanitizer.out",
  exit_code: 1,
});

itest!(junit {
  args: "test --reporter junit test/pass.ts",
  output: "test/pass.junit.out",
});

itest!(junit_nested {
  args: "test --reporter junit test/nested_failures.ts",
  output: "test/nested_failures.junit.out",
  exit_code: 1,
});

itest!(junit_multiple_test_files {
  args: "test --reporter junit test/pass.ts test/fail.ts",
  output: "test/junit_multiple_test_files.junit.out",
  exit_code: 1,
});

itest!(junit_strip_ansi {
  args: "test --reporter junit test/fail_color.ts",
  output: "test/junit_strip_ansi.junit.out",
  exit_code: 1,
});

#[test]
fn junit_path() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("test.js", "Deno.test('does test', () => {});");
  let output = context
    .new_command()
    .args("test --junit-path=sub_dir/output.xml test.js")
    .run();
  output.skip_output_check();
  output.assert_exit_code(0);
  temp_dir
    .path()
    .join("sub_dir/output.xml")
    .assert_matches_text("<?xml [WILDCARD]");
}

itest!(clear_timeout {
  args: "test test/clear_timeout.ts",
  exit_code: 0,
  output: "test/clear_timeout.out",
});

itest!(hide_empty_suites {
  args: "test --filter none test/pass.ts",
  exit_code: 0,
  output: "test/hide_empty_suites.out",
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

itest!(steps_dot_passing_steps {
  args: "test --reporter=dot test/steps/passing_steps.ts",
  exit_code: 0,
  output: "test/steps/passing_steps.dot.out",
});

itest!(steps_dot_failing_steps {
  args: "test --reporter=dot test/steps/failing_steps.ts",
  exit_code: 1,
  output: "test/steps/failing_steps.dot.out",
});

itest!(steps_dot_ignored_steps {
  args: "test --reporter=dot test/steps/ignored_steps.ts",
  exit_code: 0,
  output: "test/steps/ignored_steps.dot.out",
});

itest!(steps_tap_passing_steps {
  args: "test --reporter=tap test/steps/passing_steps.ts",
  exit_code: 0,
  output: "test/steps/passing_steps.tap.out",
});

itest!(steps_tap_failing_steps {
  args: "test --reporter=tap test/steps/failing_steps.ts",
  exit_code: 1,
  envs: vec![("NO_COLOR".to_owned(), "1".to_owned())],
  output: "test/steps/failing_steps.tap.out",
});

itest!(steps_tap_ignored_steps {
  args: "test --reporter=tap test/steps/ignored_steps.ts",
  exit_code: 0,
  output: "test/steps/ignored_steps.tap.out",
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

itest!(before_unload_prevent_default {
  args: "test --quiet test/before_unload_prevent_default.ts",
  output: "test/before_unload_prevent_default.out",
});

#[test]
fn captured_output() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("test --allow-run --allow-read test/captured_output.ts")
    .env("NO_COLOR", "1")
    .run();

  let output_start = "------- output -------";
  let output_end = "----- output end -----";
  output.assert_exit_code(0);
  let output_text = output.combined_output();
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
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("test test/recursive_permissions_pledge.js")
    .run();
  output.assert_exit_code(1);
  assert_contains!(
    output.combined_output(),
    "pledge test permissions called before restoring previous pledge"
  );
}

#[test]
fn file_protocol() {
  let file_url =
    Url::from_file_path(util::testdata_path().join("test/file_protocol.ts"))
      .unwrap()
      .to_string();

  TestContext::default()
    .new_command()
    .args_vec(["test", file_url.as_str()])
    .run()
    .assert_matches_file("test/file_protocol.out");
}

itest!(uncaught_errors {
  args: "test --quiet test/uncaught_errors_1.ts test/uncaught_errors_2.ts test/uncaught_errors_3.ts",
  output: "test/uncaught_errors.out",
  exit_code: 1,
});

itest!(report_error {
  args: "test --quiet test/report_error.ts",
  output: "test/report_error.out",
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

#[test]
// todo(#18480): re-enable
#[ignore]
fn sigint_with_hanging_test() {
  util::with_pty(
    &[
      "test",
      "--quiet",
      "--no-check",
      "test/sigint_with_hanging_test.ts",
    ],
    |mut console| {
      std::thread::sleep(std::time::Duration::from_secs(1));
      console.write_line("\x03");
      let text = console.read_until("hanging_test.ts:10:15");
      wildcard_match(
        include_str!("../testdata/test/sigint_with_hanging_test.out"),
        &text,
      );
    },
  );
}

itest!(package_json_basic {
  args: "test",
  output: "package_json/basic/lib.test.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  cwd: Some("package_json/basic"),
  copy_temp_dir: Some("package_json/basic"),
  exit_code: 0,
});

itest!(test_replace_timers {
  args: "test test/replace_timers.js",
  output: "test/replace_timers.js.out",
});

#[test]
fn test_with_glob_config() {
  let context = TestContextBuilder::new().cwd("test").build();

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/nested/fizz/fizz.ts");
  assert_contains!(output, "glob/pages/[id].ts");
  assert_contains!(output, "glob/nested/fizz/bar.ts");
  assert_contains!(output, "glob/nested/foo/foo.ts");
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/nested/foo/bar.ts");
  assert_contains!(output, "glob/nested/foo/fizz.ts");
  assert_contains!(output, "glob/nested/fizz/foo.ts");
  assert_contains!(output, "glob/data/test1.ts");
}

#[test]
fn test_with_glob_config_and_flags() {
  let context = TestContextBuilder::new().cwd("test").build();

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json --ignore=glob/nested/**/bar.ts")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/nested/fizz/fizz.ts");
  assert_contains!(output, "glob/pages/[id].ts");
  assert_contains!(output, "glob/nested/fizz/bazz.ts");
  assert_contains!(output, "glob/nested/foo/foo.ts");
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/nested/foo/bazz.ts");
  assert_contains!(output, "glob/nested/foo/fizz.ts");
  assert_contains!(output, "glob/nested/fizz/foo.ts");
  assert_contains!(output, "glob/data/test1.ts");

  let cmd_output = context
    .new_command()
    .args("test --config deno.glob.json glob/data/test1.?s")
    .run();

  cmd_output.assert_exit_code(0);

  let output = cmd_output.combined_output();
  assert_contains!(output, "glob/data/test1.js");
  assert_contains!(output, "glob/data/test1.ts");
}

#[test]
fn conditionally_loads_type_graph() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("test --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch_no_follow_with_options - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("test --reload -L debug --no-check run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}

#[test]
fn opt_out_top_level_exclude_via_test_unexclude() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "test": {
      "exclude": [ "!excluded.test.ts" ]
    },
    "exclude": [ "excluded.test.ts", "actually_excluded.test.ts" ]
  }));

  temp_dir
    .join("main.test.ts")
    .write("Deno.test('test1', () => {});");
  temp_dir
    .join("excluded.test.ts")
    .write("Deno.test('test2', () => {});");
  temp_dir
    .join("actually_excluded.test.ts")
    .write("Deno.test('test3', () => {});");

  let output = context.new_command().arg("test").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.test.ts");
  assert_contains!(output, "excluded.test.ts");
  assert_not_contains!(output, "actually_excluded.test.ts");
}
