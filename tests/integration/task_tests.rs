// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

// use test_util::env_vars_for_npm_tests;
// use test_util::itest;
// use test_util::TestContext;

// TODO(2.0): this should first run `deno install`
// itest!(task_package_json_npm_bin {
//   args: "task bin extra",
//   cwd: Some("task/package_json/"),
//   output: "task/package_json/bin.out",
//   copy_temp_dir: Some("task/package_json/"),
//   envs: env_vars_for_npm_tests(),
//   exit_code: 0,
//   http_server: true,
// });

// TODO(2.0): not entirely clear what's wrong with this test but it hangs for more than 60s
// itest!(task_npx_on_own {
//   args: "task on-own",
//   cwd: Some("task/npx/"),
//   output: "task/npx/on_own.out",
//   copy_temp_dir: Some("task/npx/"),
//   envs: env_vars_for_npm_tests(),
//   exit_code: 1,
//   http_server: true,
// });
