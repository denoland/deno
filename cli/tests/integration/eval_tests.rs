// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn eval_p() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("-p")
    .arg("1+2")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap().trim());
  assert_eq!("3", stdout_str);
}

itest!(_029_eval {
  args: "eval console.log(\"hello\")",
  output: "029_eval.out",
});

// Ugly parentheses due to whitespace delimiting problem.
itest!(_030_eval_ts {
  args: "eval --quiet --ext=ts console.log((123)as(number))", // 'as' is a TS keyword only
  output: "030_eval_ts.out",
});

itest!(_041_dyn_import_eval {
  args: "eval import('./subdir/mod4.js').then(console.log)",
  output: "041_dyn_import_eval.out",
});

// Cannot write the expression to evaluate as "console.log(typeof gc)"
// because itest! splits args on whitespace.
itest!(v8_flags_eval {
  args: "eval --v8-flags=--expose-gc console.log(typeof(gc))",
  output: "v8_flags.js.out",
});
