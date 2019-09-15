// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::process::Command;

pub fn run_python_script(script: &str) -> bool {
  let mut child = Command::new("python")
    .current_dir("../../")
    .arg(script)
    .spawn()
    .expect("failed to spawn script");

  let ecode = child.wait().expect("failed to wait for the child process");

  ecode.success()
}

// TODO(#2933): Rewrite these tests in rust.
#[test]
fn tty_tests() {
  // FIXME: These tests can't run in parallel.
  assert!(run_python_script("tools/complex_permissions_test.py"));
  assert!(run_python_script("tools/is_tty_test.py"));
  assert!(run_python_script("tools/permission_prompt_test.py"));
}
