// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::process::Command;

use deno_core::serde_json::json;
use test_util::assert_contains;
use test_util::assert_not_contains;
use test_util::env_vars_for_jsr_provenance_tests;
use test_util::env_vars_for_jsr_tests;
use test_util::env_vars_for_jsr_tests_with_git_check;
use test_util::TestContextBuilder;

#[test]
fn publish_non_exported_files_using_import_map() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
    "imports": {
      "@denotest/add": "jsr:@denotest/add@1"
    }
  }));
  // file not in the graph
  let other_ts = temp_dir.join("_other.ts");
  other_ts
    .write("import { add } from '@denotest/add'; console.log(add(1, 3));");
  let mod_ts = temp_dir.join("mod.ts");
  mod_ts.write("import { add } from '@denotest/add'; console.log(add(1, 2));");
  let output = context
    .new_command()
    .args("publish --log-level=debug --token 'sadfasdf'")
    .run();
  output.assert_exit_code(0);
  let lines = output.combined_output().split('\n').collect::<Vec<_>>();
  eprintln!("{}", output.combined_output());
  assert!(lines
    .iter()
    .any(|l| l.contains("Unfurling") && l.ends_with("mod.ts")));
  assert!(lines
    .iter()
    .any(|l| l.contains("Unfurling") && l.ends_with("other.ts")));
}

#[test]
fn publish_warning_not_in_graph() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./mod.ts",
  }));
  // file not in the graph that uses a non-analyzable dynamic import (cause a diagnostic)
  let other_ts = temp_dir.join("_other.ts");
  other_ts
    .write("const nonAnalyzable = './_other.ts'; await import(nonAnalyzable);");
  let mod_ts = temp_dir.join("mod.ts");
  mod_ts.write(
    "export function test(a: number, b: number): number { return a + b; }",
  );
  context
    .new_command()
    .args("publish --token 'sadfasdf'")
    .run()
    .assert_matches_text(
      "[WILDCARD]unable to analyze dynamic import[WILDCARD]",
    );
}

#[test]
fn provenance() {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_provenance_tests())
    .cwd("publish/successful")
    .build()
    .new_command()
    .args("publish")
    .run()
    .assert_exit_code(0)
    .assert_matches_file("publish/successful_provenance.out");
}

#[test]
fn ignores_gitignore() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts"
  }));

  temp_dir.join("main.ts").write("import './sub_dir/b.ts';");

  let gitignore = temp_dir.join(".gitignore");
  gitignore.write("ignored.ts\nsub_dir/ignored.wasm");

  let sub_dir = temp_dir.join("sub_dir");
  sub_dir.create_dir_all();
  sub_dir.join("ignored.wasm").write("");
  sub_dir.join("b.ts").write("export default {}");

  temp_dir.join("ignored.ts").write("");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--dry-run")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "b.ts");
  assert_contains!(output, "main.ts");
  assert_not_contains!(output, "ignored.ts");
  assert_not_contains!(output, "ignored.wasm");
  assert_not_contains!(output, ".gitignore");
}

#[test]
fn ignores_directories() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exclude": [ "ignore" ],
    "publish": {
      "exclude": [ "ignore2" ]
    },
    "exports": "./main_included.ts"
  }));

  let ignored_dirs = vec![
    temp_dir.join(".git"),
    temp_dir.join("node_modules"),
    temp_dir.join("ignore"),
    temp_dir.join("ignore2"),
  ];
  for ignored_dir in ignored_dirs {
    ignored_dir.create_dir_all();
    ignored_dir.join("ignored.ts").write("");
  }

  let sub_dir = temp_dir.join("sub_dir");
  sub_dir.create_dir_all();
  sub_dir.join("sub_included.ts").write("");

  temp_dir.join("main_included.ts").write("");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--log-level=debug")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "sub_included.ts");
  assert_contains!(output, "main_included.ts");
  assert_not_contains!(output, "ignored.ts");
}

#[test]
fn not_include_gitignored_file_unless_exact_match_in_include() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
    "publish": {
      // won't match ignored.ts because it needs to be
      // unexcluded via a negated glob in exclude
      "include": [
        "deno.json",
        "*.ts",
        "exact_include.ts",
        "sub"
      ]
    }
  }));

  temp_dir
    .join(".gitignore")
    .write("ignored.ts\nexact_include.ts\nsub/\nsub/ignored\n/sub_ignored\n");
  temp_dir.join("main.ts").write("");
  temp_dir.join("ignored.ts").write("");
  temp_dir.join("exact_include.ts").write("");
  let sub_dir = temp_dir.join("sub");
  sub_dir.create_dir_all();
  sub_dir.join("sub_included.ts").write("");
  sub_dir.join("ignored.ts").write(""); // this one is gitignored
  sub_dir.join("ignored").create_dir_all();
  sub_dir.join("ignored").join("ignored_also.ts").write("");
  let sub_ignored_dir = temp_dir.join("sub_ignored");
  sub_ignored_dir.create_dir_all();
  sub_ignored_dir.join("sub_ignored.ts").write("");

  let output = context.new_command().arg("publish").arg("--dry-run").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  // will match this exact match
  assert_contains!(output, "exact_include.ts");
  // will include this because the sub directory is included
  assert_contains!(output, "sub_included.ts");
  // it's gitignored
  assert_not_contains!(output, "ignored.ts");
  assert_not_contains!(output, "ignored_also.ts");
  assert_not_contains!(output, "sub_ignored.ts");
}

#[test]
fn gitignore_everything_exlcuded_override() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();

  temp_dir.join(".gitignore").write("*\n");
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./root_main.ts",
    "publish": {
      // should opt out of .gitignore even though everything
      // is .gitignored
      "exclude": ["!**"]
    }
  }));

  temp_dir.join("root_main.ts").write("");
  let sub_dir = temp_dir.join("sub");
  sub_dir.create_dir_all();
  sub_dir.join("sub_main.ts").write("");
  let output = context.new_command().arg("publish").arg("--dry-run").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "root_main.ts");
  assert_contains!(output, "sub_main.ts");
}

#[test]
fn includes_directories_with_gitignore_when_unexcluded() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
    "publish": {
      "include": [ "deno.json", "*.ts" ],
      "exclude": [ "!ignored.ts" ]
    }
  }));

  temp_dir.join(".gitignore").write("ignored.ts");
  temp_dir.join("main.ts").write("");
  temp_dir.join("ignored.ts").write("");

  let output = context.new_command().arg("publish").arg("--dry-run").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_contains!(output, "ignored.ts");
}

#[test]
fn includes_unexcluded_sub_dir() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./included1.ts",
    "publish": {
      "exclude": [
        "ignored",
        "!ignored/unexcluded",
      ]
    }
  }));

  temp_dir.join("included1.ts").write("");
  temp_dir.join("ignored/unexcluded").create_dir_all();
  temp_dir.join("ignored/ignored.ts").write("");
  temp_dir.join("ignored/unexcluded/included2.ts").write("");

  let output = context.new_command().arg("publish").arg("--dry-run").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "included1.ts");
  assert_contains!(output, "included2.ts");
  assert_not_contains!(output, "ignored.ts");
}

#[test]
fn includes_directories() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
    "publish": {
      "include": [ "deno.json", "main.ts" ]
    }
  }));

  temp_dir.join("main.ts").write("");
  temp_dir.join("ignored.ts").write("");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_not_contains!(output, "ignored.ts");
}

#[test]
fn not_includes_gitignored_dotenv() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
  }));

  temp_dir.join("main.ts").write("");
  temp_dir.join(".env").write("FOO=BAR");
  temp_dir.join(".gitignore").write(".env");

  let output = context.new_command().arg("publish").arg("--dry-run").run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "main.ts");
  assert_not_contains!(output, ".env");
}

#[test]
fn not_includes_vendor_dir_only_when_vendor_true() {
  let context = publish_context_builder().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
  }));

  temp_dir.join("main.ts").write("");
  let vendor_folder = temp_dir.join("vendor");
  vendor_folder.create_dir_all();
  vendor_folder.join("vendor.ts").write("");

  let publish_cmd = context.new_command().args("publish --dry-run");
  {
    let output = publish_cmd.run();
    output.assert_exit_code(0);
    let output = output.combined_output();
    assert_contains!(output, "main.ts");
    assert_contains!(output, "vendor.ts");
  }

  // with vendor
  {
    temp_dir.join("deno.json").write_json(&json!({
      "name": "@foo/bar",
      "version": "1.0.0",
      "exports": "./main.ts",
      "vendor": true,
    }));
    let output = publish_cmd.run();
    output.assert_exit_code(0);
    let output = output.combined_output();
    assert_contains!(output, "main.ts");
    assert_not_contains!(output, "vendor.ts");
  }
}

#[test]
fn allow_dirty() {
  let context = publish_context_builder_with_git_checks().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
  }));

  temp_dir.join("main.ts").write("");

  let cmd = Command::new("git")
    .arg("init")
    .arg(temp_dir.as_path())
    .output()
    .unwrap();
  assert!(cmd.status.success());

  let output = context
    .new_command()
    .arg("publish")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "Aborting due to uncommitted changes. Check in source code or run with --allow-dirty");

  let output = context
    .new_command()
    .arg("publish")
    .arg("--allow-dirty")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Successfully published");
}

#[test]
fn allow_dirty_not_in_repo() {
  let context = publish_context_builder_with_git_checks().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
  }));

  temp_dir.join("main.ts").write("");
  // At this point there are untracked files, but we're not in Git repo,
  // so we should be able to publish successfully.

  let output = context
    .new_command()
    .arg("publish")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(0);
  let output = output.combined_output();
  assert_contains!(output, "Successfully published");
}

#[test]
fn allow_dirty_dry_run() {
  let context = publish_context_builder_with_git_checks().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("deno.json").write_json(&json!({
    "name": "@foo/bar",
    "version": "1.0.0",
    "exports": "./main.ts",
  }));

  temp_dir.join("main.ts").write("");

  let cmd = Command::new("git")
    .arg("init")
    .arg(temp_dir.as_path())
    .output()
    .unwrap();
  assert!(cmd.status.success());

  let output = context
    .new_command()
    .arg("publish")
    .arg("--dry-run")
    .arg("--token")
    .arg("sadfasdf")
    .run();
  output.assert_exit_code(1);
  let output = output.combined_output();
  assert_contains!(output, "Aborting due to uncommitted changes. Check in source code or run with --allow-dirty");
}

fn publish_context_builder() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_tests())
    .use_temp_cwd()
}

fn publish_context_builder_with_git_checks() -> TestContextBuilder {
  TestContextBuilder::new()
    .use_http_server()
    .envs(env_vars_for_jsr_tests_with_git_check())
    .use_temp_cwd()
}
