// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::env_vars_for_jsr_tests;
use test_util::env_vars_for_npm_tests;
use test_util::itest;
use test_util::TestContextBuilder;

#[test]
fn add_basic() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  }));

  let output = context.new_command().args("add @david/which").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@david/which");
  let deno_json_content = temp_dir.join("deno.json").read_to_string();
  assert_contains!(
    deno_json_content,
    "\"imports\": {\n    \"@david/which\": \"jsr:@david/which^1\""
  );
}

fn pm_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_tests())
    .use_temp_cwd()
}
