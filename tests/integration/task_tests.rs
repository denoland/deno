// Copyright 2018-2026 the Deno authors. MIT license.

// Most of the tests for this are in deno_task_shell.
// These tests are intended to only test integration.

use test_util as util;
use util::TestContextBuilder;
use util::test;

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
