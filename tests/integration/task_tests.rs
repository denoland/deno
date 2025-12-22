// Copyright 2018-2025 the Deno authors. MIT license.

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

use test_util as util;
use util::TestContextBuilder;
use util::test;

// use test_util::env_vars_for_npm_tests;
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

#[test(flaky)]
fn deno_task_ansi_escape_codes() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", r#"{
  "tasks": {
    "dev": "echo 'BOOO!!!'",
    "next": "\u001b[3F\u001b[0G- dev\u001b[1E\u001b[2K    echo 'I am your friend.'"
  }
}
"#);

  context
    .new_command()
    .args_vec(["task"])
    .with_pty(|mut console| {
      console.expect("Available tasks:");
      console.expect("- dev");
      console.expect("    echo 'BOOO!!!'");
      console.expect("- next");
      console.expect("    - dev    echo 'I am your friend.'");
    });
}

#[test(flaky)]
fn deno_task_control_chars() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "deno.json",
    r#"{
  "tasks": {
    "dev": "echo 'BOOO!!!' && \r    echo hi there is my command",
    "serve": {
      "description": "this is a\tm\rangled description",
      "command": "echo hello"
    }
  }
}
"#,
  );

  context
    .new_command()
    .args_vec(["task"])
    .with_pty(|mut console| {
      console.expect("Available tasks:");
      console.expect("- dev");
      console
        .expect("    echo 'BOOO!!!' && \\r    echo hi there is my command");
      console.expect("- serve");
      console.expect("    // this is a\\tm\\rangled description");
      console.expect("    echo hello");
    });
}
