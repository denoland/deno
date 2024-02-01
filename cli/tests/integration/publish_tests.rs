// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::assert_not_contains;
use test_util::env_vars_for_npm_tests;
use test_util::TestContextBuilder;

static TEST_REGISTRY_URL: &str = "http://127.0.0.1:4250";

pub fn env_vars_for_registry() -> Vec<(String, String)> {
  vec![(
    "DENO_REGISTRY_URL".to_string(),
    TEST_REGISTRY_URL.to_string(),
  )]
}

itest!(no_token {
  args: "publish",
  cwd: Some("publish/missing_deno_json"),
  output: "publish/no_token.out",
  exit_code: 1,
});

itest!(missing_deno_json {
  args: "publish --token 'sadfasdf'",
  output: "publish/missing_deno_json.out",
  cwd: Some("publish/missing_deno_json"),
  exit_code: 1,
});

itest!(invalid_fast_check {
  args: "publish --token 'sadfasdf'",
  output: "publish/invalid_fast_check.out",
  cwd: Some("publish/invalid_fast_check"),
  exit_code: 1,
});

itest!(invalid_path {
  args: "publish --token 'sadfasdf'",
  output: "publish/invalid_path.out",
  cwd: Some("publish/invalid_path"),
  exit_code: 1,
});

itest!(symlink {
  args: "publish --token 'sadfasdf' --dry-run",
  output: "publish/symlink.out",
  cwd: Some("publish/symlink"),
  exit_code: 0,
});

itest!(invalid_import {
  args: "publish --token 'sadfasdf' --dry-run",
  output: "publish/invalid_import.out",
  cwd: Some("publish/invalid_import"),
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});

itest!(javascript_missing_decl_file {
  args: "publish --token 'sadfasdf'",
  output: "publish/javascript_missing_decl_file.out",
  cwd: Some("publish/javascript_missing_decl_file"),
  envs: env_vars_for_registry(),
  exit_code: 0,
  http_server: true,
});

itest!(unanalyzable_dynamic_import {
  args: "publish --token 'sadfasdf'",
  output: "publish/unanalyzable_dynamic_import.out",
  cwd: Some("publish/unanalyzable_dynamic_import"),
  envs: env_vars_for_registry(),
  exit_code: 0,
  http_server: true,
});

itest!(javascript_decl_file {
  args: "publish --token 'sadfasdf'",
  output: "publish/javascript_decl_file.out",
  cwd: Some("publish/javascript_decl_file"),
  envs: env_vars_for_registry(),
  http_server: true,
  exit_code: 0,
});

itest!(successful {
  args: "publish --token 'sadfasdf'",
  output: "publish/successful.out",
  cwd: Some("publish/successful"),
  envs: env_vars_for_registry(),
  http_server: true,
});

itest!(node_specifier {
  args: "publish --token 'sadfasdf'",
  output: "publish/node_specifier.out",
  cwd: Some("publish/node_specifier"),
  envs: env_vars_for_registry().into_iter().chain(env_vars_for_npm_tests().into_iter()).collect(),
  http_server: true,
});

itest!(config_file_jsonc {
  args: "publish --token 'sadfasdf'",
  output: "publish/deno_jsonc.out",
  cwd: Some("publish/deno_jsonc"),
  envs: env_vars_for_registry(),
  http_server: true,
});

itest!(workspace_all {
  args: "publish --token 'sadfasdf'",
  output: "publish/workspace.out",
  cwd: Some("publish/workspace"),
  envs: env_vars_for_registry(),
  http_server: true,
});

itest!(workspace_individual {
  args: "publish --token 'sadfasdf'",
  output: "publish/workspace_individual.out",
  cwd: Some("publish/workspace/bar"),
  envs: env_vars_for_registry(),
  http_server: true,
});

itest!(dry_run {
  args: "publish --token 'sadfasdf' --dry-run",
  cwd: Some("publish/successful"),
  output: "publish/dry_run.out",
  envs: env_vars_for_registry(),
  http_server: true,
});

#[test]
fn ignores_directories() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exclude": [ "ignore" ],
    "publish": {
      "exclude": [ "ignore2" ]
    },
    "exports": "./main_included.ts"
  }));

  let ignored_dirs = vec![
    temp_dir.join(".git"),
    temp_dir.join("node_modules"),
    temp_dir.join("ignore"),
    temp_dir.join("ignore2"),
  ];
  for ignored_dir in ignored_dirs {
    ignored_dir.create_dir_all();
    ignored_dir.join("ignored.ts").write("");
  }

  let sub_dir = temp_dir.join("sub_dir");
  sub_dir.create_dir_all();
  sub_dir.join("sub_included.ts").write("");

  temp_dir.join("main_included.ts").write("");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--log-level=debug")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "sub_included.ts");
  assert_contains!(output, "main_included.ts");
  assert_not_contains!(output, "ignored.ts");
}

#[test]
fn includes_directories() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
    "publish": {
      "include": [ "deno.json", "main.ts" ]
    }
  }));

  temp_dir.join("main.ts").write("");
  temp_dir.join("ignored.ts").write("");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--log-level=debug")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_not_contains!(output, "ignored.ts");
}

fn publish_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_registry())
    .use_temp_cwd()
}
