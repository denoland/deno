// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

itest!(task_no_args {
  args: "task -q --config task/deno.json",
  output: "task/task_no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_cwd {
  args: "task -q --config task/deno.json --cwd .. echo_cwd",
  output: "task/task_cwd.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 0,
});

itest!(task_init_cwd {
  args: "task -q --config task/deno.json --cwd .. echo_init_cwd",
  output: "task/task_init_cwd.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 0,
});

itest!(task_init_cwd_already_set {
  args: "task -q --config task/deno.json echo_init_cwd",
  output: "task/task_init_cwd_already_set.out",
  envs: vec![
    ("NO_COLOR".to_string(), "1".to_string()),
    ("INIT_CWD".to_string(), "HELLO".to_string())
  ],
  exit_code: 0,
});

itest!(task_cwd_resolves_config_from_specified_dir {
  args: "task -q --cwd task",
  output: "task/task_no_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_non_existent {
  args: "task --config task/deno.json non_existent",
  output: "task/task_non_existent.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(task_boolean_logic {
  args: "task -q --config task/deno.json boolean_logic",
  output: "task/task_boolean_logic.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_exit_code_5 {
  args: "task --config task/deno.json exit_code_5",
  output: "task/task_exit_code_5.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 5,
});

itest!(task_additional_args {
  args: "task -q --config task/deno.json echo 2",
  output: "task/task_additional_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_no_shell_expansion {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno.json",
    "echo",
    "$(echo 5)"
  ],
  output: "task/task_additional_args_no_shell_expansion.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_nested_strings {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno.json",
    "echo",
    "string \"quoted string\""
  ],
  output: "task/task_additional_args_nested_strings.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_additional_args_no_logic {
  args_vec: vec![
    "task",
    "-q",
    "--config",
    "task/deno.json",
    "echo",
    "||",
    "echo",
    "5"
  ],
  output: "task/task_additional_args_no_logic.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(task_deno_exe_no_env {
  args_vec: vec!["task", "-q", "--config", "task/deno.json", "deno_echo"],
  output: "task/task_deno_exe_no_env.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  env_clear: true,
});

itest!(task_piped_stdin {
  args_vec: vec!["task", "-q", "--config", "task/deno.json", "piped"],
  output: "task/task_piped_stdin.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});
