// Copyright 2018-2026 the Deno authors. MIT license.

use test_util as util;
use test_util::assert_contains;
use test_util::assert_not_contains;
use test_util::test;

// Make sure that snapshot flags don't affect runtime.
#[test]
fn eval_randomness() {
  let mut numbers = Vec::with_capacity(10);
  for _ in 0..10 {
    let output = util::deno_cmd()
      .arg("eval")
      .arg("-p")
      .arg("Math.random()")
      .stdout_piped()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    assert!(output.status.success());
    let stdout_str = util::strip_ansi_codes(
      std::str::from_utf8(&output.stdout).unwrap().trim(),
    );
    numbers.push(stdout_str.to_string());
  }
  numbers.dedup();
  assert!(numbers.len() > 1);
}

#[test]
fn eval_error_getter_does_not_panic() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg(
      r#"Object.defineProperty(Error.prototype, "name", { get() { throw new Error("getter boom"); }, configurable: true }); throw new Error("outer");"#,
    )
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr =
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap().trim());
  assert_contains!(stderr, "error: Uncaught");
  assert_not_contains!(stderr, "Deno has panicked", "panicked at");
}
