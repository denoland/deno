// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use test_util as util;
use test_util::itest;
use util::assert_contains;
use util::assert_not_contains;
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

  let fixed_ipynb = testdata_fmt_dir.join("badly_formatted_fixed.ipynb");
  let badly_formatted_original_ipynb =
    testdata_fmt_dir.join("badly_formatted.ipynb");
  let badly_formatted_ipynb = t.path().join("badly_formatted.ipynb");
  badly_formatted_original_ipynb.copy(&badly_formatted_ipynb);

  let fixed_yaml = testdata_fmt_dir.join("badly_formatted_fixed.yaml");
  let badly_formatted_original_yaml =
    testdata_fmt_dir.join("badly_formatted.yaml");
  let badly_formatted_yaml = t.path().join("badly_formatted.yaml");
  badly_formatted_original_yaml.copy(&badly_formatted_yaml);

  // First, check formatting by ignoring the badly formatted file.
  let output = context
    .new_command()
    .current_dir(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      format!(
        "--ignore={badly_formatted_js},{badly_formatted_md},{badly_formatted_json},{badly_formatted_yaml},{badly_formatted_ipynb}",
      ),
      format!(
        "--check {badly_formatted_js} {badly_formatted_md} {badly_formatted_json} {badly_formatted_yaml} {badly_formatted_ipynb}",
      ),
    ])
    .run();

  // No target files found
  output.assert_exit_code(1);
  output.skip_output_check();

  // Check without ignore.
  let output = context
    .new_command()
    .current_dir(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      "--check".to_string(),
      badly_formatted_js.to_string(),
      badly_formatted_md.to_string(),
      badly_formatted_json.to_string(),
      badly_formatted_yaml.to_string(),
      badly_formatted_ipynb.to_string(),
    ])
    .run();

  output.assert_exit_code(1);
  output.skip_output_check();

  // Format the source file.
  let output = context
    .new_command()
    .current_dir(&testdata_fmt_dir)
    .args_vec(vec![
      "fmt".to_string(),
      badly_formatted_js.to_string(),
      badly_formatted_md.to_string(),
      badly_formatted_json.to_string(),
      badly_formatted_yaml.to_string(),
      badly_formatted_ipynb.to_string(),
    ])
    .run();

  output.assert_exit_code(0);
  output.skip_output_check();

  let expected_js = fixed_js.read_to_string();
  let expected_md = fixed_md.read_to_string();
  let expected_json = fixed_json.read_to_string();
  let expected_yaml = fixed_yaml.read_to_string();
  let expected_ipynb = fixed_ipynb.read_to_string();
  let actual_js = badly_formatted_js.read_to_string();
  let actual_md = badly_formatted_md.read_to_string();
  let actual_json = badly_formatted_json.read_to_string();
  let actual_yaml = badly_formatted_yaml.read_to_string();
  let actual_ipynb = badly_formatted_ipynb.read_to_string();
  assert_eq!(expected_js, actual_js);
  assert_eq!(expected_md, actual_md);
  assert_eq!(expected_json, actual_json);
  assert_eq!(expected_yaml, actual_yaml);
  assert_eq!(expected_ipynb, actual_ipynb);
}

#[test]
fn fmt_stdin_syntax_error() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg("-")
    .stdin_text("import { example }")
    .split_output()
    .run();
  assert!(output.stdout().is_empty());
  assert!(!output.stderr().is_empty());
  output.assert_exit_code(1);
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

  let context = TestContext::default();
  let temp_dir = context.temp_dir();
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

  let output = context
    .new_command()
    .current_dir(t)
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

itest!(fmt_stdin_ipynb {
  args: "fmt --ext=ipynb -",
  input: Some(include_str!("../testdata/fmt/badly_formatted.ipynb")),
  output_str: Some(include_str!("../testdata/fmt/badly_formatted_fixed.ipynb")),
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

#[test]
fn opt_out_top_level_exclude_via_fmt_unexclude() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "fmt": {
      "exclude": [ "!excluded.ts" ]
    },
    "exclude": [ "excluded.ts", "actually_excluded.ts" ]
  }));

  temp_dir.join("main.ts").write("const a   = 1;");
  temp_dir.join("excluded.ts").write("const a   = 2;");
  temp_dir
    .join("actually_excluded.ts")
    .write("const a   = 2;");

  let output = context.new_command().arg("fmt").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_contains!(output, "excluded.ts");
  assert_not_contains!(output, "actually_excluded.ts");
}
