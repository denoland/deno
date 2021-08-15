// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn no_color() {
  let (out, _) = util::run_and_collect_output(
    false,
    "test test/no_color.ts",
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

itest!(meta {
  args: "test test/meta.ts",
  exit_code: 0,
  output: "test/meta.out",
});

itest!(pass {
  args: "test test/pass.ts",
  exit_code: 0,
  output: "test/pass.out",
});

itest!(ignore {
  args: "test test/ignore.ts",
  exit_code: 0,
  output: "test/ignore.out",
});

itest!(ignore_permissions {
  args: "test --unstable test/ignore_permissions.ts",
  exit_code: 0,
  output: "test/ignore_permissions.out",
});

itest!(fail {
  args: "test test/fail.ts",
  exit_code: 1,
  output: "test/fail.out",
});

itest!(load_unload {
  args: "test test/load_unload.ts",
  exit_code: 0,
  output: "test/load_unload.out",
});

itest!(doc {
  args: "test --doc --allow-all test/doc.ts",
  exit_code: 1,
  output: "test/doc.out",
});

itest!(markdown {
  args: "test --doc --allow-all test/markdown.md",
  exit_code: 1,
  output: "test/markdown.out",
});

itest!(text {
  args: "test --doc --allow-all test/text.md",
  exit_code: 0,
  output: "test/text.out",
});

itest!(quiet {
  args: "test --quiet test/quiet.ts",
  exit_code: 0,
  output: "test/quiet.out",
});

itest!(fail_fast {
  args: "test --fail-fast test/fail_fast.ts",
  exit_code: 1,
  output: "test/fail_fast.out",
});

itest!(only {
  args: "test test/only.ts",
  exit_code: 1,
  output: "test/only.out",
});

itest!(no_check {
  args: "test --no-check test/no_check.ts",
  exit_code: 1,
  output: "test/no_check.out",
});

itest!(no_run {
  args: "test --unstable --no-run test/no_run.ts",
  output: "test/no_run.out",
  exit_code: 1,
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

itest!(exit_sanitizer {
  args: "test test/exit_sanitizer.ts",
  output: "test/exit_sanitizer.out",
  exit_code: 1,
});

itest!(clear_timeout {
  args: "test test/clear_timeout.ts",
  exit_code: 0,
  output: "test/clear_timeout.out",
});

itest!(finally_timeout {
  args: "test test/finally_timeout.ts",
  exit_code: 1,
  output: "test/finally_timeout.out",
});

itest!(unresolved_promise {
  args: "test test/unresolved_promise.ts",
  exit_code: 1,
  output: "test/unresolved_promise.out",
});

itest!(unhandled_rejection {
  args: "test test/unhandled_rejection.ts",
  exit_code: 1,
  output: "test/unhandled_rejection.out",
});

itest!(filter {
  args: "test --filter=foo test/filter",
  exit_code: 0,
  output: "test/filter.out",
});

itest!(shuffle {
  args: "test --shuffle test/shuffle",
  exit_code: 0,
  output_str: Some("[WILDCARD]"),
});

itest!(shuffle_with_seed {
  args: "test --shuffle=42 test/shuffle",
  exit_code: 0,
  output: "test/shuffle.out",
});
