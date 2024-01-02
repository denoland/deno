// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

use deno_core::serde_json::json;
use test_util::env_vars_for_npm_tests;
use test_util::TestContext;
use test_util::TestContextBuilder;

itest!(task_no_args {
  args: "task -q --config task/deno_json/deno.json",
  output: "task/deno_json/task_no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_cwd {
  args: "task -q --config task/deno_json/deno.json --cwd .. echo_cwd",
  output: "task/deno_json/task_cwd.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 0,
});

itest!(task_init_cwd {
  args: "task -q --config task/deno_json/deno.json --cwd .. echo_init_cwd",
  output: "task/deno_json/task_init_cwd.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 0,
});

itest!(task_init_cwd_already_set {
  args: "task -q --config task/deno_json/deno.json echo_init_cwd",
  output: "task/deno_json/task_init_cwd_already_set.out",
  envs: vec![
    ("NO_COLOR".to_string(), "1".to_string()),
    ("INIT_CWD".to_string(), "HELLO".to_string())
  ],
  exit_code: 0,
});

itest!(task_cwd_resolves_config_from_specified_dir {
  args: "task -q --cwd task/deno_json",
  output: "task/deno_json/task_no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_non_existent {
  args: "task --config task/deno_json/deno.json non_existent",
  output: "task/deno_json/task_non_existent.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

#[test]
fn task_emoji() {
  // this bug only appears when using a pty/tty
  TestContext::default()
    .new_command()
    .args_vec(["task", "--config", "task/deno_json/deno.json", "echo_emoji"])
    .with_pty(|mut console| {
      console.expect("Task echo_emoji echo 🔥\r\n🔥");
    });
}

itest!(task_boolean_logic {
  args: "task -q --config task/deno_json/deno.json boolean_logic",
  output: "task/deno_json/task_boolean_logic.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_exit_code_5 {
  args: "task --config task/deno_json/deno.json exit_code_5",
  output: "task/deno_json/task_exit_code_5.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 5,
});

itest!(task_additional_args {
  args: "task -q --config task/deno_json/deno.json echo 2",
  output: "task/deno_json/task_additional_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_no_shell_expansion {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno_json/deno.json",
    "echo",
    "$(echo 5)"
  ],
  output: "task/deno_json/task_additional_args_no_shell_expansion.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_nested_strings {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno_json/deno.json",
    "echo",
    "string \"quoted string\""
  ],
  output: "task/deno_json/task_additional_args_nested_strings.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_no_logic {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno_json/deno.json",
    "echo",
    "||",
    "echo",
    "5"
  ],
  output: "task/deno_json/task_additional_args_no_logic.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_deno_exe_no_env {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno_json/deno.json",
    "deno_echo"
  ],
  output: "task/deno_json/task_deno_exe_no_env.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  env_clear: true,
});

itest!(task_piped_stdin {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno_json/deno.json",
    "piped"
  ],
  output: "task/deno_json/task_piped_stdin.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_package_json_no_arg {
  args: "task",
  cwd: Some("task/package_json/"),
  output: "task/package_json/no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_package_json_echo {
  args: "task --quiet echo",
  cwd: Some("task/package_json/"),
  output: "task/package_json/echo.out",
  // use a temp dir because the node_modules folder will be created
  copy_temp_dir: Some("task/package_json/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  http_server: true,
});

itest!(task_package_json_npm_bin {
  args: "task bin extra",
  cwd: Some("task/package_json/"),
  output: "task/package_json/bin.out",
  copy_temp_dir: Some("task/package_json/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  http_server: true,
});

itest!(task_both_no_arg {
  args: "task",
  cwd: Some("task/both/"),
  output: "task/both/no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_both_deno_json_selected {
  args: "task other",
  cwd: Some("task/both/"),
  output: "task/both/deno_selected.out",
  copy_temp_dir: Some("task/both/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  http_server: true,
});

itest!(task_both_package_json_selected {
  args: "task bin asdf",
  cwd: Some("task/both/"),
  output: "task/both/package_json_selected.out",
  copy_temp_dir: Some("task/both/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  http_server: true,
});

itest!(task_both_prefers_deno {
  args: "task output some text",
  cwd: Some("task/both/"),
  output: "task/both/prefers_deno.out",
  copy_temp_dir: Some("task/both/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
  http_server: true,
});

itest!(task_npx_non_existent {
  args: "task non-existent",
  cwd: Some("task/npx/"),
  output: "task/npx/non_existent.out",
  copy_temp_dir: Some("task/npx/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});

itest!(task_npx_on_own {
  args: "task on-own",
  cwd: Some("task/npx/"),
  output: "task/npx/on_own.out",
  copy_temp_dir: Some("task/npx/"),
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});

itest!(task_pre_post {
  args: "task test",
  cwd: Some("task/package_json_pre_post/"),
  output: "task/package_json_pre_post/bin.out",
  copy_temp_dir: Some("task/package_json_pre_post/"),
  exit_code: 0,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_pre {
  args: "task test",
  cwd: Some("task/package_json_pre/"),
  output: "task/package_json_pre/bin.out",
  copy_temp_dir: Some("task/package_json_pre/"),
  exit_code: 0,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_post {
  args: "task test",
  cwd: Some("task/package_json_post/"),
  output: "task/package_json_post/bin.out",
  copy_temp_dir: Some("task/package_json_post/"),
  exit_code: 0,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_post_only {
  args: "task test",
  cwd: Some("task/package_json_post_only/"),
  output: "task/package_json_post_only/bin.out",
  copy_temp_dir: Some("task/package_json_post_only/"),
  exit_code: 1,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_pre_only {
  args: "task test",
  cwd: Some("task/package_json_pre_only/"),
  output: "task/package_json_pre_only/bin.out",
  copy_temp_dir: Some("task/package_json_pre_only/"),
  exit_code: 1,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_deno_no_pre_post {
  args: "task test",
  cwd: Some("task/deno_json_pre_post/"),
  output: "task/deno_json_pre_post/bin.out",
  copy_temp_dir: Some("task/deno_json_pre_post/"),
  exit_code: 0,
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

#[test]
fn task_byonm() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("package.json").write_json(&json!({
    "name": "example",
    "scripts": {
      "say": "cowsay 'do make say'",
      "think": "cowthink think"
    },
    "dependencies": {
      "cowsay": "*"
    }
  }));
  temp_dir.join("deno.json").write_json(&json!({
    "unstable": ["byonm"],
  }));
  context.run_npm("install");

  context
    .new_command()
    .args_vec(["task", "say"])
    .run()
    .assert_matches_text(
      r#"Task say cowsay 'do make say'
 _____________
< do make say >
 -------------
        \   ^__^
         \  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||
"#,
    );

  context
    .new_command()
    .args_vec(["task", "think"])
    .run()
    .assert_matches_text(
      r#"Task think cowthink think
 _______
( think )
 -------
        o   ^__^
         o  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||
"#,
    );
}
