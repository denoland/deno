// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;

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
