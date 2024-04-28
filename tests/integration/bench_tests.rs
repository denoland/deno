// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use deno_core::url::Url;
use test_util as util;
use test_util::itest;
use test_util::itest_flaky;
use util::assert_contains;
use util::assert_not_contains;
use util::env_vars_for_npm_tests;
use util::TestContext;
use util::TestContextBuilder;















itest!(collect {
  args: "bench --ignore=bench/collect/ignore bench/collect",
  exit_code: 0,
  output: "bench/collect.out",
});































itest!(filter {
  args: "bench --filter=foo bench/filter",
  exit_code: 0,
  output: "bench/filter.out",
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

#[test]
fn conditionally_loads_type_graph() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("bench --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch_no_follow_with_options - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("bench --reload -L debug --no-check run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}

#[test]
fn opt_out_top_level_exclude_via_bench_unexclude() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "bench": {
      "exclude": [ "!excluded.bench.ts" ]
    },
    "exclude": [ "excluded.bench.ts", "actually_excluded.bench.ts" ]
  }));

  temp_dir
    .join("main.bench.ts")
    .write("Deno.bench('test1', () => {});");
  temp_dir
    .join("excluded.bench.ts")
    .write("Deno.bench('test2', () => {});");
  temp_dir
    .join("actually_excluded.bench.ts")
    .write("Deno.bench('test3', () => {});");

  let output = context.new_command().arg("bench").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.bench.ts");
  assert_contains!(output, "excluded.bench.ts");
  assert_not_contains!(output, "actually_excluded.bench.ts");
}
