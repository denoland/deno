// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::io::Read;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;

use test_util::deno_exe_path;
use test_util::testdata_path;
use test_util::TempDir;

use crate::itest;

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

itest!(task_no_args {
  args: "task -q --config task/deno.json",
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

#[test]
fn task_subprocess_kill() {
  let t = TempDir::new();
  let mut new_deno_exe_path = t.path().join("temp_deno_task_wait_exit");
  if cfg!(windows) {
    new_deno_exe_path.set_extension("exe");
  }
  std::fs::copy(deno_exe_path(), new_deno_exe_path).unwrap();

  kill_processes_with_name("temp_deno_task_wait_exit");

  let mut child = Command::new(deno_exe_path())
    .env("PATH", t.path().to_string_lossy().to_string())
    .args(["task", "--quiet", "wait_exit"])
    .current_dir(testdata_path().join("task"))
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();

  // give task some time to start up
  let mut stdout = child.stdout.take().unwrap();
  let mut buf = [0; 7];
  stdout.read_exact(&mut buf).unwrap();
  assert_eq!(std::str::from_utf8(&buf).unwrap(), "started");

  // now kill it
  child.kill().unwrap();

  // now wait a bit and ensure the child process was killed
  std::thread::sleep(Duration::from_millis(10));
  assert!(!kill_processes_with_name("temp_deno_task_wait_exit"));
}

#[cfg(windows)]
fn kill_processes_with_name(name: &str) -> bool {
  let name = format!("{}.exe", name);
  Command::new("taskkill")
    .args(["/f", "/t", "/im", &name])
    .status()
    .unwrap()
    .success()
}

#[cfg(not(windows))]
fn kill_processes_with_name(name: &str) -> bool {
  Command::new("pkill")
    .args([name])
    .status()
    .unwrap()
    .success()
}
