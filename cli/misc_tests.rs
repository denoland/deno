// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![cfg(test)]

use std::process::Command;

fn run_python_script(script: &str) -> bool {
  let mut child = Command::new("python")
    .current_dir("../")
    .arg(script)
    .spawn()
    .expect("failed to spawn script");

  let ecode = child.wait().expect("failed to wait for the child process");

  ecode.success()
}

#[test]
fn benchmark_test() {
  assert!(run_python_script("tools/benchmark_test.py"))
}

#[test]
fn deno_dir_test() {
  let g = crate::test_http_server::run();
  assert!(run_python_script("tools/deno_dir_test.py"));
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fetch_test() {
  let g = crate::test_http_server::run();
  assert!(run_python_script("tools/fetch_test.py"));
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fmt_test() {
  assert!(run_python_script("tools/fmt_test.py"))
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn js_unit_tests() {
  let g = crate::test_http_server::run();
  assert!(run_python_script("tools/unit_tests.py"));
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn repl_test() {
  assert!(run_python_script("tools/repl_test.py"))
}

#[test]
fn setup_test() {
  assert!(run_python_script("tools/setup_test.py"))
}

#[test]
fn target_test() {
  assert!(run_python_script("tools/target_test.py"))
}

#[test]
fn util_test() {
  assert!(run_python_script("tools/util_test.py"))
}
