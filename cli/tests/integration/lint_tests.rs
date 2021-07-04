// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn ignore_unexplicit_files() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .env("NO_COLOR", "1")
    .arg("lint")
    .arg("--ignore=./")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(
    String::from_utf8_lossy(&output.stderr),
    "error: No target files found.\n"
  );
}

itest!(all {
  args: "lint lint/file1.js lint/file2.ts lint/ignored_file.ts",
  output: "lint/expected.out",
  exit_code: 1,
});

itest!(quiet {
  args: "lint --quiet lint/file1.js",
  output: "lint/expected_quiet.out",
  exit_code: 1,
});

itest!(json {
  args:
    "lint --json lint/file1.js lint/file2.ts lint/ignored_file.ts lint/malformed.js",
  output: "lint/expected_json.out",
  exit_code: 1,
});

itest!(ignore {
  args: "lint --ignore=lint/file1.js,lint/malformed.js lint/",
  output: "lint/expected_ignore.out",
  exit_code: 1,
});

itest!(glob {
  args: "lint --ignore=lint/malformed.js lint/",
  output: "lint/expected_glob.out",
  exit_code: 1,
});

itest!(stdin {
  args: "lint -",
  input: Some("let _a: any;"),
  output: "lint/expected_from_stdin.out",
  exit_code: 1,
});

itest!(stdin_json {
  args: "lint --json -",
  input: Some("let _a: any;"),
  output: "lint/expected_from_stdin_json.out",
  exit_code: 1,
});

itest!(rules {
  args: "lint --rules",
  output: "lint/expected_rules.out",
  exit_code: 0,
});

// Make sure that the rules are printed if quiet option is enabled.
itest!(rules_quiet {
  args: "lint --rules -q",
  output: "lint/expected_rules.out",
  exit_code: 0,
});

itest!(rule_doc_signle {
  args: "lint --rule no-empty",
  output: "lint/expected_rule_doc_single.out",
  exit_code: 0,
});

itest!(rule_doc_unknown_rule_specified {
  args: "lint --rule __UNKNOWN_RULE_NAME__",
  output: "lint/expected_rule_doc_unknown_rule_specified.out",
  exit_code: 1,
});

itest!(rule_doc_double {
  args: "lint --rule no-empty no-debugger",
  output: "lint/expected_rule_doc_double.out",
  exit_code: 0,
});

itest!(rule_doc_signle_json {
  args: "lint --rule no-empty --json",
  output: "lint/expected_rule_doc_single_json.out",
  exit_code: 0,
});
