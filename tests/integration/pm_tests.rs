// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::env_vars_for_jsr_npm_tests;
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

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
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

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  // Don't use `assert_matches_json` to ensure the file is properly formatted.
  let expected = r#"{
  "imports": {
    "@denotest/add": "jsr:@denotest/add@^1.0.0"
  }
}
"#;
  temp_dir.join("deno.json").assert_matches_text(expected);
}

#[test]
fn add_basic_with_empty_deno_json() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "");

  let output = context.new_command().args("add jsr:@denotest/add").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir
    .path()
    .join("deno.json")
    .assert_matches_json(json!({
      "imports": {
        "@denotest/add": "jsr:@denotest/add@^1.0.0"
      }
    }));
}

#[test]
fn add_version_contraint() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add jsr:@denotest/add@1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@^1.0.0"
    }
  }));
}

#[test]
fn add_tilde() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add jsr:@denotest/add@~1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "@denotest/add": "jsr:@denotest/add@~1.0.0"
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
    .args("add jsr:@denotest/add jsr:@denotest/subset-type-graph")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add jsr:@denotest/add");
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
fn add_npm() {
  let context = pm_context_builder().build();
  let temp_dir = context.temp_dir().path();

  let output = context.new_command().args("add npm:chalk@4.1").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Add npm:chalk");
  temp_dir.join("deno.json").assert_matches_json(json!({
    "imports": {
      "chalk": "npm:chalk@^4.1.2"
    }
  }));
}

fn pm_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_npm_tests())
    .use_temp_cwd()
}
