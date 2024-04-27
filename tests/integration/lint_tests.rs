// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::assert_not_contains;
use test_util::itest;
use test_util::TestContextBuilder;

itest!(ignore_unexplicit_files {
  args: "lint --ignore=./",
  output_str: Some("error: No target files found.\n"),
  exit_code: 1,
});

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

itest!(compact {
  args:
    "lint --compact lint/without_config/file1.js lint/without_config/ignored_file.tss",
    output: "lint/expected_compact.out",
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

itest!(lint_with_report_config {
  args: "lint --config lint/Deno.compact.format.jsonc lint/with_config/",
  output: "lint/with_report_config_compact.out",
  exit_code: 1,
});

// Check if CLI flags take precedence
itest!(lint_with_report_config_override {
  args: "lint --config lint/Deno.compact.format.jsonc lint/with_config/ --json",
  output: "lint/with_report_config_override.out",
  exit_code: 1,
});

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

#[test]
fn lint_with_glob_config() {
  let context = TestContextBuilder::new().cwd("lint").build();

  let cmd_output = context
    .new_command()
    .args("lint --config deno.glob.json")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\nested\fizz\fizz.ts:1:10");
    assert_contains!(output, r"glob\pages\[id].ts:1:10");
    assert_contains!(output, r"glob\nested\fizz\bar.ts:1:10");
    assert_contains!(output, r"glob\nested\foo\foo.ts:1:10");
    assert_contains!(output, r"glob\data\test1.js:1:10");
    assert_contains!(output, r"glob\nested\foo\bar.ts:1:10");
    assert_contains!(output, r"glob\nested\foo\fizz.ts:1:10");
    assert_contains!(output, r"glob\nested\fizz\foo.ts:1:10");
    assert_contains!(output, r"glob\data\test1.ts:1:10");
  } else {
    assert_contains!(output, "glob/nested/fizz/fizz.ts:1:10");
    assert_contains!(output, "glob/pages/[id].ts:1:10");
    assert_contains!(output, "glob/nested/fizz/bar.ts:1:10");
    assert_contains!(output, "glob/nested/foo/foo.ts:1:10");
    assert_contains!(output, "glob/data/test1.js:1:10");
    assert_contains!(output, "glob/nested/foo/bar.ts:1:10");
    assert_contains!(output, "glob/nested/foo/fizz.ts:1:10");
    assert_contains!(output, "glob/nested/fizz/foo.ts:1:10");
    assert_contains!(output, "glob/data/test1.ts:1:10");
  }
  assert_contains!(output, "Found 9 problems");
  assert_contains!(output, "Checked 9 files");
}

#[test]
fn lint_with_glob_config_and_flags() {
  let context = TestContextBuilder::new().cwd("lint").build();

  let cmd_output = context
    .new_command()
    .args("lint --config deno.glob.json --ignore=glob/nested/**/bar.ts")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\nested\fizz\fizz.ts:1:10");
    assert_contains!(output, r"glob\pages\[id].ts:1:10");
    assert_contains!(output, r"glob\nested\fizz\bazz.ts:1:10");
    assert_contains!(output, r"glob\nested\foo\foo.ts:1:10");
    assert_contains!(output, r"glob\data\test1.js:1:10");
    assert_contains!(output, r"glob\nested\foo\bazz.ts:1:10");
    assert_contains!(output, r"glob\nested\foo\fizz.ts:1:10");
    assert_contains!(output, r"glob\nested\fizz\foo.ts:1:10");
    assert_contains!(output, r"glob\data\test1.ts:1:10");
  } else {
    assert_contains!(output, "glob/nested/fizz/fizz.ts:1:10");
    assert_contains!(output, "glob/pages/[id].ts:1:10");
    assert_contains!(output, "glob/nested/fizz/bazz.ts:1:10");
    assert_contains!(output, "glob/nested/foo/foo.ts:1:10");
    assert_contains!(output, "glob/data/test1.js:1:10");
    assert_contains!(output, "glob/nested/foo/bazz.ts:1:10");
    assert_contains!(output, "glob/nested/foo/fizz.ts:1:10");
    assert_contains!(output, "glob/nested/fizz/foo.ts:1:10");
    assert_contains!(output, "glob/data/test1.ts:1:10");
  }
  assert_contains!(output, "Found 9 problems");
  assert_contains!(output, "Checked 9 files");

  let cmd_output = context
    .new_command()
    .args("lint --config deno.glob.json glob/data/test1.?s")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\data\test1.js:1:10");
    assert_contains!(output, r"glob\data\test1.ts:1:10");
  } else {
    assert_contains!(output, "glob/data/test1.js:1:10");
    assert_contains!(output, "glob/data/test1.ts:1:10");
  }
  assert_contains!(output, "Found 2 problems");
  assert_contains!(output, "Checked 2 files");
}

#[test]
fn opt_out_top_level_exclude_via_lint_unexclude() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "lint": {
      "exclude": [ "!excluded.ts" ]
    },
    "exclude": [ "excluded.ts", "actually_excluded.ts" ]
  }));

  temp_dir.join("main.ts").write("const a = 1;");
  temp_dir.join("excluded.ts").write("const a = 2;");
  temp_dir.join("actually_excluded.ts").write("const a = 2;");

  let output = context.new_command().arg("lint").run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_contains!(output, "excluded.ts");
  assert_not_contains!(output, "actually_excluded.ts");
}
