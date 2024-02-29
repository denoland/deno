// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::env_vars_for_jsr_tests;
// use test_util::env_vars_for_npm_tests;
// use test_util::itest;
use test_util::TestContextBuilder;

#[test]
fn add_basic() {
  let starting_deno_json = json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  });
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&starting_deno_json);

  let output = context.new_command().args("add @denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add @denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0"
    }
  }));
}

#[test]
fn add_basic_no_deno_json() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add @denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add @denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0"
    }
  }));
}

#[test]
fn add_multiple() {
  let starting_deno_json = json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  });
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&starting_deno_json);

  let output = context
    .new_command()
    .args("add @denotest/add @denotest/subset-type-graph")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add @denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0",
      "@denotest/subset-type-graph": "jsr:@denotest/subset-type-graph@^0.1.0"
    }
  }));
}

#[test]
fn add_not_supported_npm() {
  let context = pm_context_builder().build();

  let output = context
    .new_command()
    .args("add @denotest/add npm:express")
    .run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "error: Adding npm: packages is currently not supported. Package: npm:express");
}

#[test]
fn add_not_supported_version_constraint() {
  let context = pm_context_builder().build();

  let output = context.new_command().args("add @denotest/add@1").run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "error: Specifying version constraints is currently not supported. Package: jsr:@denotest/add@1");
}

fn pm_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_tests())
    .use_temp_cwd()
}
