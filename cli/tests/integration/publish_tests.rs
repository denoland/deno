// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::assert_not_contains;
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
  copy_temp_dir: Some("publish/missing_deno_json"),
  exit_code: 1,
  temp_cwd: true,
});

itest!(invalid_fast_check {
  args: "publish --token 'sadfasdf'",
  output: "publish/invalid_fast_check.out",
  cwd: Some("publish/invalid_fast_check"),
  copy_temp_dir: Some("publish/invalid_fast_check"),
  exit_code: 1,
  temp_cwd: true,
});

itest!(javascript_missing_decl_file {
  args: "publish --token 'sadfasdf'",
  output: "publish/javascript_missing_decl_file.out",
  cwd: Some("publish/javascript_missing_decl_file"),
  copy_temp_dir: Some("publish/javascript_missing_decl_file"),
  envs: env_vars_for_registry(),
  exit_code: 0,
  http_server: true,
  temp_cwd: true,
});

itest!(javascript_decl_file {
  args: "publish --token 'sadfasdf'",
  output: "publish/javascript_decl_file.out",
  cwd: Some("publish/javascript_decl_file"),
  copy_temp_dir: Some("publish/javascript_decl_file"),
  envs: env_vars_for_registry(),
  http_server: true,
  exit_code: 0,
  temp_cwd: true,
});

itest!(successful {
  args: "publish --token 'sadfasdf'",
  output: "publish/successful.out",
  cwd: Some("publish/successful"),
  copy_temp_dir: Some("publish/successful"),
  envs: env_vars_for_registry(),
  http_server: true,
  temp_cwd: true,
});

itest!(workspace_all {
  args: "publish --token 'sadfasdf'",
  output: "publish/workspace.out",
  cwd: Some("publish/workspace"),
  copy_temp_dir: Some("publish/workspace"),
  envs: env_vars_for_registry(),
  http_server: true,
  temp_cwd: true,
});

itest!(workspace_individual {
  args: "publish --token 'sadfasdf'",
  output: "publish/workspace_individual.out",
  cwd: Some("publish/workspace/bar"),
  copy_temp_dir: Some("publish/workspace"),
  envs: env_vars_for_registry(),
  http_server: true,
  temp_cwd: true,
});

itest!(dry_run {
  args: "publish --token 'sadfasdf' --dry-run",
  cwd: Some("publish/successful"),
  copy_temp_dir: Some("publish/successful"),
  output: "publish/dry_run.out",
  envs: env_vars_for_registry(),
  http_server: true,
  temp_cwd: true,
});

#[test]
fn ignores_directories() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exclude": [ "ignore" ],
    "exports": "./main_included.ts"
  }));

  let ignored_dirs = vec![
    temp_dir.join(".git"),
    temp_dir.join("node_modules"),
    temp_dir.join("ignore"),
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

fn publish_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_registry())
    .use_temp_cwd()
}
