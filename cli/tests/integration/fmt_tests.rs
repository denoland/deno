// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::TempDir;
use util::assert_contains;
use util::PathRef;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn fmt_test() {
  let context = TestContext::default();
  let t = context.deno_dir();
  let testdata_fmt_dir = util::testdata_path().join("fmt");
  let fixed_js = testdata_fmt_dir.join("badly_formatted_fixed.js");
  let badly_formatted_original_js =
    testdata_fmt_dir.join("badly_formatted.mjs");
  let badly_formatted_js = t.path().join("badly_formatted.js");
  badly_formatted_original_js.copy(&badly_formatted_js);

  let fixed_md = testdata_fmt_dir.join("badly_formatted_fixed.md");
  let badly_formatted_original_md = testdata_fmt_dir.join("badly_formatted.md");
  let badly_formatted_md = t.path().join("badly_formatted.md");
  badly_formatted_original_md.copy(&badly_formatted_md);

  let fixed_json = testdata_fmt_dir.join("badly_formatted_fixed.json");
  let badly_formatted_original_json =
    testdata_fmt_dir.join("badly_formatted.json");
  let badly_formatted_json = t.path().join("badly_formatted.json");
  badly_formatted_original_json.copy(&badly_formatted_json);
  // First, check formatting by ignoring the badly formatted file.

  let output = context
    .new_command()
    .cwd(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      format!(
        "--ignore={badly_formatted_js},{badly_formatted_md},{badly_formatted_json}",
      ),
      format!(
        "--check {badly_formatted_js} {badly_formatted_md} {badly_formatted_json}",
      ),
    ])
    .run();

  // No target files found
  output.assert_exit_code(1);
  output.skip_output_check();

  // Check without ignore.
  let output = context
    .new_command()
    .cwd(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      "--check".to_string(),
      badly_formatted_js.to_string(),
      badly_formatted_md.to_string(),
      badly_formatted_json.to_string(),
    ])
    .run();

  output.assert_exit_code(1);
  output.skip_output_check();

  // Format the source file.
  let output = context
    .new_command()
    .cwd(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      badly_formatted_js.to_string(),
      badly_formatted_md.to_string(),
      badly_formatted_json.to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let expected_js = fixed_js.read_to_string();
  let expected_md = fixed_md.read_to_string();
  let expected_json = fixed_json.read_to_string();
  let actual_js = badly_formatted_js.read_to_string();
  let actual_md = badly_formatted_md.read_to_string();
  let actual_json = badly_formatted_json.read_to_string();
  assert_eq!(expected_js, actual_js);
  assert_eq!(expected_md, actual_md);
  assert_eq!(expected_json, actual_json);
}

#[test]
fn fmt_stdin_error() {
  use std::io::Write;
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let stdin = deno.stdin.as_mut().unwrap();
  let invalid_js = b"import { example }";
  stdin.write_all(invalid_js).unwrap();
  let output = deno.wait_with_output().unwrap();
  // Error message might change. Just check stdout empty, stderr not.
  assert!(output.stdout.is_empty());
  assert!(!output.stderr.is_empty());
  assert!(!output.status.success());
}

#[test]
fn fmt_ignore_unexplicit_files() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("fmt --check --ignore=./")
    .run();

  output.assert_exit_code(1);
  assert_eq!(output.combined_output(), "error: No target files found.\n");
}

#[test]
fn fmt_auto_ignore_git_and_node_modules() {
  fn create_bad_json(t: PathRef) {
    let bad_json_path = t.join("bad.json");
    bad_json_path.write("bad json\n");
  }

  let temp_dir = TempDir::new();
  let t = temp_dir.path().join("target");
  let nest_git = t.join("nest").join(".git");
  let git_dir = t.join(".git");
  let nest_node_modules = t.join("nest").join("node_modules");
  let node_modules_dir = t.join("node_modules");
  nest_git.create_dir_all();
  git_dir.create_dir_all();
  nest_node_modules.create_dir_all();
  node_modules_dir.create_dir_all();
  create_bad_json(nest_git);
  create_bad_json(git_dir);
  create_bad_json(nest_node_modules);
  create_bad_json(node_modules_dir);

  let context = TestContext::default();
  let output = context
    .new_command()
    .cwd(t)
    .env("NO_COLOR", "1")
    .args("fmt")
    .run();

  output.assert_exit_code(1);
  assert_eq!(output.combined_output(), "error: No target files found.\n");
}

itest!(fmt_quiet_check_fmt_dir {
  args: "fmt --check --quiet fmt/regular/",
  output_str: Some(""),
  exit_code: 0,
});

itest!(fmt_check_formatted_files {
  args: "fmt --check fmt/regular/formatted1.js fmt/regular/formatted2.ts fmt/regular/formatted3.markdown fmt/regular/formatted4.jsonc",
  output: "fmt/expected_fmt_check_formatted_files.out",
  exit_code: 0,
});

itest!(fmt_check_ignore {
  args: "fmt --check --ignore=fmt/regular/formatted1.js fmt/regular/",
  output: "fmt/expected_fmt_check_ignore.out",
  exit_code: 0,
});

itest!(fmt_check_parse_error {
  args: "fmt --check fmt/parse_error/parse_error.ts",
  output: "fmt/fmt_check_parse_error.out",
  exit_code: 1,
});

itest!(fmt_check_invalid_data {
  args: "fmt --check fmt/invalid_data.json",
  output: "fmt/invalid_data.out",
  exit_code: 1,
});

itest!(fmt_stdin {
  args: "fmt -",
  input: Some("const a = 1\n"),
  output_str: Some("const a = 1;\n"),
});

itest!(fmt_stdin_markdown {
  args: "fmt --ext=md -",
  input: Some("# Hello      Markdown\n```ts\nconsole.log( \"text\")\n```\n\n```cts\nconsole.log( 5 )\n```"),
  output_str: Some("# Hello Markdown\n\n```ts\nconsole.log(\"text\");\n```\n\n```cts\nconsole.log(5);\n```\n"),
});

itest!(fmt_stdin_json {
  args: "fmt --ext=json -",
  input: Some("{    \"key\":   \"value\"}"),
  output_str: Some("{ \"key\": \"value\" }\n"),
});

itest!(fmt_stdin_check_formatted {
  args: "fmt --check -",
  input: Some("const a = 1;\n"),
  output_str: Some(""),
});

itest!(fmt_stdin_check_not_formatted {
  args: "fmt --check -",
  input: Some("const a = 1\n"),
  output_str: Some("Not formatted stdin\n"),
});

itest!(fmt_with_config {
  args: "fmt --config fmt/with_config/deno.jsonc fmt/with_config/subdir",
  output: "fmt/fmt_with_config.out",
});

itest!(fmt_with_deprecated_config {
  args:
    "fmt --config fmt/with_config/deno.deprecated.jsonc fmt/with_config/subdir",
  output: "fmt/fmt_with_deprecated_config.out",
});

itest!(fmt_with_config_default {
  args: "fmt fmt/with_config/subdir",
  output: "fmt/fmt_with_config.out",
});

// Check if CLI flags take precedence
itest!(fmt_with_config_and_flags {
  args: "fmt --config fmt/with_config/deno.jsonc --ignore=fmt/with_config/subdir/a.ts,fmt/with_config/subdir/b.ts",
  output: "fmt/fmt_with_config_and_flags.out",
});

itest!(fmt_with_malformed_config {
  args: "fmt --config fmt/deno.malformed.jsonc",
  output: "fmt/fmt_with_malformed_config.out",
  exit_code: 1,
});

itest!(fmt_with_malformed_config2 {
  args: "fmt --config fmt/deno.malformed2.jsonc",
  output: "fmt/fmt_with_malformed_config2.out",
  exit_code: 1,
});

#[test]
fn fmt_with_glob_config() {
  let context = TestContextBuilder::new().cwd("fmt").build();

  let cmd_output = context
    .new_command()
    .args("fmt --check --config deno.glob.json")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\nested\fizz\fizz.ts");
    assert_contains!(output, r"glob\pages\[id].ts");
    assert_contains!(output, r"glob\nested\fizz\bar.ts");
    assert_contains!(output, r"glob\nested\foo\foo.ts");
    assert_contains!(output, r"glob\data\test1.js");
    assert_contains!(output, r"glob\nested\foo\bar.ts");
    assert_contains!(output, r"glob\nested\foo\fizz.ts");
    assert_contains!(output, r"glob\nested\fizz\foo.ts");
    assert_contains!(output, r"glob\data\test1.ts");
  } else {
    assert_contains!(output, "glob/nested/fizz/fizz.ts");
    assert_contains!(output, "glob/pages/[id].ts");
    assert_contains!(output, "glob/nested/fizz/bar.ts");
    assert_contains!(output, "glob/nested/foo/foo.ts");
    assert_contains!(output, "glob/data/test1.js");
    assert_contains!(output, "glob/nested/foo/bar.ts");
    assert_contains!(output, "glob/nested/foo/fizz.ts");
    assert_contains!(output, "glob/nested/fizz/foo.ts");
    assert_contains!(output, "glob/data/test1.ts");
  }

  assert_contains!(output, "Found 9 not formatted files in 9 files");
}

#[test]
fn fmt_with_glob_config_and_flags() {
  let context = TestContextBuilder::new().cwd("fmt").build();

  let cmd_output = context
    .new_command()
    .args("fmt --check --config deno.glob.json --ignore=glob/nested/**/bar.ts")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\nested\fizz\fizz.ts");
    assert_contains!(output, r"glob\pages\[id].ts");
    assert_contains!(output, r"glob\nested\fizz\bazz.ts");
    assert_contains!(output, r"glob\nested\foo\foo.ts");
    assert_contains!(output, r"glob\data\test1.js");
    assert_contains!(output, r"glob\nested\foo\bazz.ts");
    assert_contains!(output, r"glob\nested\foo\fizz.ts");
    assert_contains!(output, r"glob\nested\fizz\foo.ts");
    assert_contains!(output, r"glob\data\test1.ts");
  } else {
    assert_contains!(output, "glob/nested/fizz/fizz.ts");
    assert_contains!(output, "glob/pages/[id].ts");
    assert_contains!(output, "glob/nested/fizz/bazz.ts");
    assert_contains!(output, "glob/nested/foo/foo.ts");
    assert_contains!(output, "glob/data/test1.js");
    assert_contains!(output, "glob/nested/foo/bazz.ts");
    assert_contains!(output, "glob/nested/foo/fizz.ts");
    assert_contains!(output, "glob/nested/fizz/foo.ts");
    assert_contains!(output, "glob/data/test1.ts");
  }
  assert_contains!(output, "Found 9 not formatted files in 9 files");
  let cmd_output = context
    .new_command()
    .args("fmt --check --config deno.glob.json glob/data/test1.?s")
    .run();

  cmd_output.assert_exit_code(1);

  let output = cmd_output.combined_output();
  if cfg!(windows) {
    assert_contains!(output, r"glob\data\test1.js");
    assert_contains!(output, r"glob\data\test1.ts");
  } else {
    assert_contains!(output, "glob/data/test1.js");
    assert_contains!(output, "glob/data/test1.ts");
  }

  assert_contains!(output, "Found 2 not formatted files in 2 files");
}
