// Copyright 2018-2026 the Deno authors. MIT license.

use test_util as util;
use util::TestContextBuilder;
use util::assert_contains;
use util::test;

#[test]
fn bump_version_patch() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.4.6"}"#);

  let output = context
    .new_command()
    .args("bump-version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.4.7");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.4.7""#);
}

#[test]
fn bump_version_minor() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.4.6"}"#);

  let output = context
    .new_command()
    .args("bump-version minor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.5.0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.5.0""#);
}

#[test]
fn bump_version_major() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "2.5.9"}"#);

  let output = context
    .new_command()
    .args("bump-version major")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "3.0.0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "3.0.0""#);
}

#[test]
fn bump_version_premajor() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.4.6"}"#);

  let output = context
    .new_command()
    .args("bump-version premajor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "2.0.0-0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "2.0.0-0""#);
}

#[test]
fn bump_version_preminor() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.4.6"}"#);

  let output = context
    .new_command()
    .args("bump-version preminor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.5.0-0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.5.0-0""#);
}

#[test]
fn bump_version_prepatch() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.4.6"}"#);

  let output = context
    .new_command()
    .args("bump-version prepatch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.4.7-0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.4.7-0""#);
}

#[test]
fn bump_version_prerelease_from_stable() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);

  let output = context
    .new_command()
    .args("bump-version prerelease")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.0.1-0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.0.1-0""#);
}

#[test]
fn bump_version_prerelease_increments_pre_tag() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // First prerelease
  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);
  context.new_command().args("bump-version prerelease").run();

  // Second prerelease should increment the pre-release number
  let output = context
    .new_command()
    .args("bump-version prerelease")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.0.1-1");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "1.0.1-1""#);
}

#[test]
fn bump_version_preserves_deno_json_structure() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  cwd.join("deno.json").write(
    r#"{
  "name": "@example/my-package",
  "version": "0.1.0",
  "exports": "./mod.ts",
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}"#,
  );

  let output = context
    .new_command()
    .args("bump-version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "0.1.1""#);
  assert_contains!(content, r#""name": "@example/my-package""#);
  assert_contains!(content, r#""exports": "./mod.ts""#);
  assert_contains!(content, r#""tasks""#);
}

#[test]
fn bump_version_works_with_package_json() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  cwd.join("package.json").write(
    r#"{
  "name": "my-package",
  "version": "1.5.2",
  "scripts": {
    "test": "deno test"
  }
}"#,
  );

  let output = context
    .new_command()
    .args("bump-version minor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1.6.0");

  let content = cwd.join("package.json").read_to_string();
  assert_contains!(content, r#""version": "1.6.0""#);
  assert_contains!(content, r#""name": "my-package""#);
}

#[test]
fn bump_version_fails_without_config_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();

  let output = context
    .new_command()
    .args("bump-version patch")
    .split_output()
    .run();

  output.assert_exit_code(1);
  assert_contains!(output.stderr(), "No deno.json or package.json found");
}

#[test]
fn bump_version_no_args_shows_current_version() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"version": "2.3.4"}"#);

  let output = context
    .new_command()
    .args("bump-version")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "2.3.4");

  // File should not be modified
  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "2.3.4""#);
}

#[test]
fn bump_version_defaults_to_0_1_0_when_missing() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd.join("deno.json").write(r#"{"name": "test-package"}"#);

  let output = context
    .new_command()
    .args("bump-version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "0.1.1");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "0.1.1""#);
  assert_contains!(content, r#""name": "test-package""#);
}

#[test]
fn bump_version_major_clears_prerelease() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  cwd
    .join("deno.json")
    .write(r#"{"version": "1.2.3-beta.1"}"#);

  let output = context
    .new_command()
    .args("bump-version major")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "2.0.0");

  let content = cwd.join("deno.json").read_to_string();
  assert_contains!(content, r#""version": "2.0.0""#);
}
