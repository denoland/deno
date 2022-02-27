// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
  args: "lint lint/without_config/file1.js lint/without_config/file2.ts lint/without_config/ignored_file.ts",
  output: "lint/expected.out",
  exit_code: 1,
});

itest!(quiet {
  args: "lint --quiet lint/without_config/file1.js",
  output: "lint/expected_quiet.out",
  exit_code: 1,
});

itest!(json {
  args:
    "lint --json lint/without_config/file1.js lint/without_config/file2.ts lint/without_config/ignored_file.ts lint/without_config/malformed.js",
    output: "lint/expected_json.out",
    exit_code: 1,
});

itest!(ignore {
  args:
    "lint --ignore=lint/without_config/file1.js,lint/without_config/malformed.js,lint/without_config/lint_with_config/ lint/without_config/",
  output: "lint/expected_ignore.out",
  exit_code: 1,
});

itest!(glob {
  args: "lint --ignore=lint/without_config/malformed.js,lint/with_config/ lint/without_config/",
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

itest!(lint_with_config {
  args: "lint --config lint/Deno.jsonc lint/with_config/",
  output: "lint/with_config.out",
  exit_code: 1,
});

// Check if CLI flags take precedence
itest!(lint_with_config_and_flags {
  args: "lint --config lint/Deno.jsonc --ignore=lint/with_config/a.ts",
  output: "lint/with_config_and_flags.out",
  exit_code: 1,
});

itest!(lint_with_config_without_tags {
  args: "lint --config lint/Deno.no_tags.jsonc lint/with_config/",
  output: "lint/with_config_without_tags.out",
  exit_code: 1,
});

itest!(lint_with_malformed_config {
  args: "lint --config lint/Deno.malformed.jsonc",
  output: "lint/with_malformed_config.out",
  exit_code: 1,
});

itest!(lint_with_malformed_config2 {
  args: "lint --config lint/Deno.malformed2.jsonc",
  output: "lint/with_malformed_config2.out",
  exit_code: 1,
});
