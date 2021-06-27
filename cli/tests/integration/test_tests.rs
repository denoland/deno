// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn no_color() {
  let (out, _) = util::run_and_collect_output(
    false,
    "test test/deno_test_no_color.ts",
    None,
    Some(vec![("NO_COLOR".to_owned(), "true".to_owned())]),
    false,
  );
  // ANSI escape codes should be stripped.
  assert!(out.contains("test success ... ok"));
  assert!(out.contains("test fail ... FAILED"));
  assert!(out.contains("test ignored ... ignored"));
  assert!(out.contains("test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out"));
}

itest!(all {
  args: "test test/test_runner_test.ts",
  exit_code: 1,
  output: "test/deno_test.out",
});

itest!(doc {
  args: "test --doc --allow-all test/doc.ts",
  exit_code: 1,
  output: "test/doc.out",
});

itest!(allow_all {
  args: "test --unstable --allow-all test/allow_all.ts",
  exit_code: 0,
  output: "test/allow_all.out",
});

itest!(allow_none {
  args: "test --unstable test/allow_none.ts",
  exit_code: 1,
  output: "test/allow_none.out",
});

itest!(fail_fast {
  args: "test --fail-fast test/test_runner_test.ts",
  exit_code: 1,
  output: "test/deno_test_fail_fast.out",
});

itest!(only {
  args: "test test/deno_test_only.ts",
  exit_code: 1,
  output: "test/deno_test_only.ts.out",
});

itest!(no_check {
  args: "test --no-check test/test_runner_test.ts",
  exit_code: 1,
  output: "test/deno_test.out",
});

itest!(finally_cleartimeout {
  args: "test test/test_finally_cleartimeout.ts",
  exit_code: 1,
  output: "test/test_finally_cleartimeout.out",
});

itest!(unresolved_promise {
  args: "test test/test_unresolved_promise.js",
  exit_code: 1,
  output: "test/deno_test_unresolved_promise.out",
});

itest!(unhandled_rejection {
  args: "test test/unhandled_rejection.ts",
  exit_code: 1,
  output: "test/unhandled_rejection.out",
});

itest!(exit_sanitizer {
  args: "test test/exit_sanitizer_test.ts",
  output: "test/exit_sanitizer_test.out",
  exit_code: 1,
});

itest!(quiet {
  args: "test --quiet test/quiet_test.ts",
  exit_code: 0,
  output: "test/quiet_test.out",
});

itest!(_067_test_no_run_type_error {
  args: "test --unstable --no-run test_type_error",
  output: "067_test_no_run_type_error.out",
  exit_code: 1,
});
