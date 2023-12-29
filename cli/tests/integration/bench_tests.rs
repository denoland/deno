// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;
use test_util as util;
use util::assert_contains;
use util::assert_not_contains;
use util::env_vars_for_npm_tests;
use util::TestContext;

itest!(overloads {
  args: "bench bench/overloads.ts",
  exit_code: 0,
  output: "bench/overloads.out",
});

itest!(meta {
  args: "bench bench/meta.ts",
  exit_code: 0,
  output: "bench/meta.out",
});

itest!(pass {
  args: "bench bench/pass.ts",
  exit_code: 0,
  output: "bench/pass.out",
});

itest!(ignore {
  args: "bench bench/ignore.ts",
  exit_code: 0,
  output: "bench/ignore.out",
});

itest!(ignore_permissions {
  args: "bench bench/ignore_permissions.ts",
  exit_code: 0,
  output: "bench/ignore_permissions.out",
});

itest!(fail {
  args: "bench bench/fail.ts",
  exit_code: 1,
  output: "bench/fail.out",
});

itest!(bench_formatting {
  args: "bench bench/bench_formatting.ts",
  exit_code: 0,
  output: "bench/bench_formatting.out",
});

itest!(collect {
  args: "bench --ignore=bench/collect/ignore bench/collect",
  exit_code: 0,
  output: "bench/collect.out",
});

itest!(load_unload {
  args: "bench bench/load_unload.ts",
  exit_code: 0,
  output: "bench/load_unload.out",
});

itest!(interval {
  args: "bench bench/interval.ts",
  exit_code: 0,
  output: "bench/interval.out",
});

itest!(quiet {
  args: "bench --quiet bench/quiet.ts",
  exit_code: 0,
  output: "bench/quiet.out",
});

itest!(only {
  args: "bench bench/only.ts",
  exit_code: 1,
  output: "bench/only.out",
});

itest!(multifile_summary {
  args: "bench bench/group_baseline.ts bench/pass.ts bench/multiple_group.ts",
  exit_code: 0,
  output: "bench/multifile_summary.out",
});

itest!(no_check {
  args: "bench --no-check bench/no_check.ts",
  exit_code: 1,
  output: "bench/no_check.out",
});

itest!(allow_all {
  args: "bench  --allow-all bench/allow_all.ts",
  exit_code: 0,
  output: "bench/allow_all.out",
});

itest!(allow_none {
  args: "bench bench/allow_none.ts",
  exit_code: 1,
  output: "bench/allow_none.out",
});

itest!(exit_sanitizer {
  args: "bench bench/exit_sanitizer.ts",
  output: "bench/exit_sanitizer.out",
  exit_code: 1,
});

itest!(clear_timeout {
  args: "bench bench/clear_timeout.ts",
  exit_code: 0,
  output: "bench/clear_timeout.out",
});

itest!(finally_timeout {
  args: "bench bench/finally_timeout.ts",
  exit_code: 1,
  output: "bench/finally_timeout.out",
});

itest!(before_unload_prevent_default {
  args: "bench --quiet bench/before_unload_prevent_default.ts",
  output: "bench/before_unload_prevent_default.out",
});

itest!(group_baseline {
  args: "bench bench/group_baseline.ts",
  exit_code: 0,
  output: "bench/group_baseline.out",
});

itest!(unresolved_promise {
  args: "bench bench/unresolved_promise.ts",
  exit_code: 1,
  output: "bench/unresolved_promise.out",
});

itest!(unhandled_rejection {
  args: "bench bench/unhandled_rejection.ts",
  exit_code: 1,
  output: "bench/unhandled_rejection.out",
});

itest!(filter {
  args: "bench --filter=foo bench/filter",
  exit_code: 0,
  output: "bench/filter.out",
});

itest!(no_run {
  args: "bench --no-run bench/no_run.ts",
  output: "bench/no_run.out",
  exit_code: 1,
});

itest!(no_prompt_by_default {
  args: "bench --quiet bench/no_prompt_by_default.ts",
  exit_code: 1,
  output: "bench/no_prompt_by_default.out",
});

itest!(no_prompt_with_denied_perms {
  args: "bench --quiet --allow-read bench/no_prompt_with_denied_perms.ts",
  exit_code: 1,
  output: "bench/no_prompt_with_denied_perms.out",
});

itest!(check_local_by_default {
  args: "bench --quiet bench/check_local_by_default.ts",
  output: "bench/check_local_by_default.out",
  http_server: true,
});

itest!(check_local_by_default2 {
  args: "bench --quiet bench/check_local_by_default2.ts",
  output: "bench/check_local_by_default2.out",
  http_server: true,
  exit_code: 1,
});

itest!(bench_explicit_start_end {
  args: "bench --quiet -A bench/explicit_start_and_end.ts",
  output: "bench/explicit_start_and_end.out",
  exit_code: 1,
});

itest_flaky!(bench_explicit_start_end_low_precision {
  args: "bench --quiet -A bench/explicit_start_and_end_low_precision.ts",
  output: "bench/explicit_start_and_end_low_precision.out",
});

itest!(bench_with_config {
  args: "bench --config bench/collect/deno.jsonc bench/collect",
  exit_code: 0,
  output: "bench/collect.out",
});

itest!(bench_with_config2 {
  args: "bench --config bench/collect/deno2.jsonc bench/collect",
  exit_code: 0,
  output: "bench/collect2.out",
});

itest!(bench_with_malformed_config {
  args: "bench --config bench/collect/deno.malformed.jsonc",
  exit_code: 1,
  output: "bench/collect_with_malformed_config.out",
});

itest!(json_output {
  args: "bench --json bench/pass.ts",
  exit_code: 0,
  output: "bench/pass.json.out",
});

#[test]
fn recursive_permissions_pledge() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("bench bench/recursive_permissions_pledge.js")
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
    Url::from_file_path(util::testdata_path().join("bench/file_protocol.ts"))
      .unwrap()
      .to_string();
  let context = TestContext::default();
  context
    .new_command()
    .args(format!("bench bench/file_protocol.ts {file_url}"))
    .run()
    .assert_matches_file("bench/file_protocol.out");
}

itest!(package_json_basic {
  args: "bench",
  output: "package_json/basic/lib.bench.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  cwd: Some("package_json/basic"),
  copy_temp_dir: Some("package_json/basic"),
  exit_code: 0,
});

itest!(bench_lock {
  args: "bench",
  http_server: true,
  cwd: Some("lockfile/basic"),
  exit_code: 10,
  output: "lockfile/basic/fail.out",
});

itest!(bench_no_lock {
  args: "bench --no-lock",
  http_server: true,
  cwd: Some("lockfile/basic"),
  output: "lockfile/basic/bench.nolock.out",
});

#[test]
fn conditionally_loads_type_graph() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("bench --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch() - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("bench --reload -L debug --no-check run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}
