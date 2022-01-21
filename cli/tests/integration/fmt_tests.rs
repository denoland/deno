// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use tempfile::TempDir;
use test_util as util;

#[test]
fn fmt_test() {
  let t = TempDir::new().expect("tempdir fail");
  let fixed_js = util::testdata_path().join("badly_formatted_fixed.js");
  let badly_formatted_original_js =
    util::testdata_path().join("badly_formatted.mjs");
  let badly_formatted_js = t.path().join("badly_formatted.js");
  let badly_formatted_js_str = badly_formatted_js.to_str().unwrap();
  std::fs::copy(&badly_formatted_original_js, &badly_formatted_js)
    .expect("Failed to copy file");

  let fixed_md = util::testdata_path().join("badly_formatted_fixed.md");
  let badly_formatted_original_md =
    util::testdata_path().join("badly_formatted.md");
  let badly_formatted_md = t.path().join("badly_formatted.md");
  let badly_formatted_md_str = badly_formatted_md.to_str().unwrap();
  std::fs::copy(&badly_formatted_original_md, &badly_formatted_md)
    .expect("Failed to copy file");

  let fixed_json = util::testdata_path().join("badly_formatted_fixed.json");
  let badly_formatted_original_json =
    util::testdata_path().join("badly_formatted.json");
  let badly_formatted_json = t.path().join("badly_formatted.json");
  let badly_formatted_json_str = badly_formatted_json.to_str().unwrap();
  std::fs::copy(&badly_formatted_original_json, &badly_formatted_json)
    .expect("Failed to copy file");
  // First, check formatting by ignoring the badly formatted file.
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg(format!(
      "--ignore={},{},{}",
      badly_formatted_js_str, badly_formatted_md_str, badly_formatted_json_str
    ))
    .arg("--check")
    .arg(badly_formatted_js_str)
    .arg(badly_formatted_md_str)
    .arg(badly_formatted_json_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  // No target files found
  assert!(!status.success());

  // Check without ignore.
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg("--check")
    .arg(badly_formatted_js_str)
    .arg(badly_formatted_md_str)
    .arg(badly_formatted_json_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(!status.success());

  // Format the source file.
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg(badly_formatted_js_str)
    .arg(badly_formatted_md_str)
    .arg(badly_formatted_json_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(status.success());
  let expected_js = std::fs::read_to_string(fixed_js).unwrap();
  let expected_md = std::fs::read_to_string(fixed_md).unwrap();
  let expected_json = std::fs::read_to_string(fixed_json).unwrap();
  let actual_js = std::fs::read_to_string(badly_formatted_js).unwrap();
  let actual_md = std::fs::read_to_string(badly_formatted_md).unwrap();
  let actual_json = std::fs::read_to_string(badly_formatted_json).unwrap();
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
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("fmt")
    .arg("--check")
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

itest!(fmt_stdin {
  args: "fmt -",
  input: Some("const a = 1\n"),
  output_str: Some("const a = 1;\n"),
});

itest!(fmt_stdin_markdown {
  args: "fmt --ext=md -",
  input: Some("# Hello      Markdown\n```ts\nconsole.log( \"text\")\n```\n"),
  output_str: Some("# Hello Markdown\n\n```ts\nconsole.log(\"text\");\n```\n"),
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
