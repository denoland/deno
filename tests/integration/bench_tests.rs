// Copyright 2018-2025 the Deno authors. MIT license.

use serde_json::json;
use test_util as util;
use util::assert_contains;
use util::assert_not_contains;
use util::TestContext;
use util::TestContextBuilder;

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
fn conditionally_loads_type_graph() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("bench --reload -L debug run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch_no_follow - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
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
