// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use test_util as util;

#[test]
fn ignore_unexplicit_files() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .env("NO_COLOR", "1")
    .arg("lint")
    .arg("--unstable")
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
  args: "lint --unstable lint/file1.js lint/file2.ts lint/ignored_file.ts",
  output: "lint/expected.out",
  exit_code: 1,
});

itest!(quiet {
  args: "lint --unstable --quiet lint/file1.js",
  output: "lint/expected_quiet.out",
  exit_code: 1,
});

itest!(json {
      args:
        "lint --unstable --json lint/file1.js lint/file2.ts lint/ignored_file.ts lint/malformed.js",
        output: "lint/expected_json.out",
        exit_code: 1,
    });

itest!(ignore {
  args: "lint --unstable --ignore=lint/file1.js,lint/malformed.js lint/",
  output: "lint/expected_ignore.out",
  exit_code: 1,
});

itest!(glob {
  args: "lint --unstable --ignore=lint/malformed.js lint/",
  output: "lint/expected_glob.out",
  exit_code: 1,
});

itest!(stdin {
  args: "lint --unstable -",
  input: Some("let _a: any;"),
  output: "lint/expected_from_stdin.out",
  exit_code: 1,
});

itest!(stdin_json {
  args: "lint --unstable --json -",
  input: Some("let _a: any;"),
  output: "lint/expected_from_stdin_json.out",
  exit_code: 1,
});

itest!(rules {
  args: "lint --unstable --rules",
  output: "lint/expected_rules.out",
  exit_code: 0,
});

// Make sure that the rules are printed if quiet option is enabled.
itest!(rules_quiet {
  args: "lint --unstable --rules -q",
  output: "lint/expected_rules.out",
  exit_code: 0,
});
