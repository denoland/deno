// Copyright 2018-2025 the Deno authors. MIT license.

use test_util as util;
use util::TestContextBuilder;
use util::assert_contains;

#[test]
fn deno_version_patch_increments_correctly() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create a deno.json with initial version
  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);

  let output = context
    .new_command()
    .args("version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.0.0 to 1.0.1");

  // Verify the file was updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.0.1""#);
}

#[test]
fn deno_version_minor_increments_correctly() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create a deno.json with initial version
  cwd.join("deno.json").write(r#"{"version": "1.2.3"}"#);

  let output = context
    .new_command()
    .args("version minor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.2.3 to 1.3.0");

  // Verify the file was updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.3.0""#);
}

#[test]
fn deno_version_major_increments_correctly() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create a deno.json with initial version
  cwd.join("deno.json").write(r#"{"version": "2.5.9"}"#);

  let output = context
    .new_command()
    .args("version major")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 2.5.9 to 3.0.0");

  // Verify the file was updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "3.0.0""#);
}

#[test]
fn deno_version_prerelease_increments_correctly() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Test initial prerelease
  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);

  let output = context
    .new_command()
    .args("version prerelease")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.0.0 to 1.0.1-0");

  // Test subsequent prerelease increment
  let output = context
    .new_command()
    .args("version prerelease")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.0.1-0 to 1.0.1-1");

  // Verify the file was updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.0.1-1""#);
}

#[test]
fn deno_version_works_with_deno_json() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create a deno.json with complex structure
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
    .args("version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 0.1.0 to 0.1.1");
  assert_contains!(output.stderr(), "Updated version in");

  // Verify the file structure is preserved
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "0.1.1""#);
  assert_contains!(deno_json_content, r#""name": "@example/my-package""#);
  assert_contains!(deno_json_content, r#""exports": "./mod.ts""#);
  assert_contains!(deno_json_content, r#""tasks""#);
}

#[test]
fn deno_version_works_with_package_json() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create a package.json
  cwd.join("package.json").write(
    r#"{
  "name": "my-package",
  "version": "1.5.2",
  "description": "A test package",
  "scripts": {
    "test": "deno test"
  }
}"#,
  );

  let output = context
    .new_command()
    .args("version minor")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.5.2 to 1.6.0");

  // Verify the file structure is preserved
  let package_json_content = cwd.join("package.json").read_to_string();
  assert_contains!(package_json_content, r#""version": "1.6.0""#);
  assert_contains!(package_json_content, r#""name": "my-package""#);
  assert_contains!(package_json_content, r#""scripts""#);
}

#[test]
fn deno_version_works_with_both_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create both deno.json and package.json
  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);
  cwd
    .join("package.json")
    .write(r#"{"name": "test", "version": "1.0.0"}"#);

  let output = context
    .new_command()
    .args("version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.0.0 to 1.0.1");

  // Verify both files were updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.0.1""#);

  let package_json_content = cwd.join("package.json").read_to_string();
  assert_contains!(package_json_content, r#""version": "1.0.1""#);
}

#[test]
fn deno_version_fails_without_config_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();

  let output = context
    .new_command()
    .args("version patch")
    .split_output()
    .run();

  output.assert_exit_code(1);
  assert_contains!(output.stderr(), "No deno.json or package.json found");
}

#[test]
fn deno_version_dry_run_shows_changes() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  cwd.join("deno.json").write(r#"{"version": "1.0.0"}"#);

  let output = context
    .new_command()
    .args("version patch --dry-run")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Current version: 1.0.0");
  assert_contains!(output.stderr(), "New version: 1.0.1");
  assert_contains!(output.stderr(), "Would update:");

  // Verify the file was NOT updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.0.0""#);
}

#[test]
fn deno_version_without_increment_shows_current() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  cwd.join("deno.json").write(r#"{"version": "2.3.4"}"#);

  let output = context.new_command().args("version").split_output().run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "2.3.4");

  // Verify the file was NOT updated
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "2.3.4""#);
}

#[test]
fn deno_version_creates_default_version_when_missing() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  // Create deno.json without version
  cwd.join("deno.json").write(r#"{"name": "test-package"}"#);

  let output = context
    .new_command()
    .args("version patch")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stderr(), "Version updated from 1.0.0 to 1.0.1");

  // Verify version was added
  let deno_json_content = cwd.join("deno.json").read_to_string();
  assert_contains!(deno_json_content, r#""version": "1.0.1""#);
  assert_contains!(deno_json_content, r#""name": "test-package""#);
}
