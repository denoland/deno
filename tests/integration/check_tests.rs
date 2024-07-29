// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::itest;
use util::env_vars_for_npm_tests;
use util::TestContext;
use util::TestContextBuilder;

itest!(check_all {
  args: "check --quiet --all check/all/check_all.ts",
  output: "check/all/check_all.out",
  http_server: true,
  exit_code: 1,
});

itest!(check_all_local {
  args: "check --quiet check/all/check_all.ts",
  output_str: Some(""),
  http_server: true,
});

itest!(module_detection_force {
  args: "check --quiet check/module_detection_force/main.ts",
  output_str: Some(""),
});

// Regression test for https://github.com/denoland/deno/issues/14937.
itest!(declaration_header_file_with_no_exports {
  args: "check --quiet check/declaration_header_file_with_no_exports.ts",
  output_str: Some(""),
});

itest!(check_jsximportsource_importmap_config {
  args: "check --quiet --config check/jsximportsource_importmap_config/deno.json check/jsximportsource_importmap_config/main.tsx",
  output_str: Some(""),
});

itest!(bundle_jsximportsource_importmap_config {
  args: "bundle --quiet --config check/jsximportsource_importmap_config/deno.json check/jsximportsource_importmap_config/main.tsx",
  output: "check/jsximportsource_importmap_config/main.bundle.js",
});

itest!(jsx_not_checked {
  args: "check check/jsx_not_checked/main.jsx",
  output: "check/jsx_not_checked/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 1,
});

itest!(check_npm_install_diagnostics {
  args: "check --quiet check/npm_install_diagnostics/main.ts",
  output: "check/npm_install_diagnostics/main.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(check_export_equals_declaration_file {
  args: "check --quiet check/export_equals_declaration_file/main.ts",
  exit_code: 0,
});

itest!(check_static_response_json {
  args: "check --quiet check/response_json.ts",
  exit_code: 0,
});

itest!(check_node_builtin_modules_ts {
  args: "check --quiet check/node_builtin_modules/mod.ts",
  output: "check/node_builtin_modules/mod.ts.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});

itest!(check_node_builtin_modules_js {
  args: "check --quiet check/node_builtin_modules/mod.js",
  output: "check/node_builtin_modules/mod.js.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
  http_server: true,
});

itest!(check_no_error_truncation {
  args: "check --quiet check/no_error_truncation/main.ts --config check/no_error_truncation/deno.json",
  output: "check/no_error_truncation/main.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
  exit_code: 1,
});

itest!(check_broadcast_channel {
  args: "check --quiet check/broadcast_channel.ts",
  exit_code: 0,
});

itest!(check_deno_not_found {
  args: "check --quiet check/deno_not_found/main.ts",
  output: "check/deno_not_found/main.out",
  exit_code: 1,
});

itest!(check_with_exclude_option_by_dir {
  args:
    "check --quiet --config check/exclude_option/deno.exclude_dir.json check/exclude_option/ignored/index.ts",
  output_str: Some(""),
  exit_code: 0,
});

itest!(check_with_exclude_option_by_glob {
  args:
    "check --quiet --config check/exclude_option/deno.exclude_glob.json check/exclude_option/ignored/index.ts",
  output_str: Some(""),
  exit_code: 0,
});

itest!(check_without_exclude_option {
  args:
    "check --quiet --config check/exclude_option/deno.json check/exclude_option/ignored/index.ts",
  output: "check/exclude_option/exclude_option.ts.error.out",
  exit_code: 1,
});

itest!(check_imported_files_listed_in_exclude_option {
  args:
    "check --quiet --config check/exclude_option/deno.exclude_dir.json check/exclude_option/index.ts",
  output: "check/exclude_option/exclude_option.ts.error.out",
  exit_code: 1,
});

#[test]
fn cache_switching_config_then_no_config() {
  let context = TestContext::default();

  assert!(does_type_checking(&context, true));
  assert!(does_type_checking(&context, false));

  // should now not do type checking even when it changes
  // configs because it previously did
  assert!(!does_type_checking(&context, true));
  assert!(!does_type_checking(&context, false));

  fn does_type_checking(context: &TestContext, with_config: bool) -> bool {
    let mut args = vec![
      "check".to_string(),
      "check/cache_config_on_off/main.ts".to_string(),
    ];
    if with_config {
      let mut slice = vec![
        "--config".to_string(),
        "check/cache_config_on_off/deno.json".to_string(),
      ];
      args.append(&mut slice);
    }

    let output = context.new_command().args_vec(args).split_output().run();

    output.assert_exit_code(0);

    let stderr = output.stderr();
    stderr.contains("Check")
  }
}

#[test]
fn reload_flag() {
  // should do type checking whenever someone specifies --reload
  let context = TestContext::default();

  assert!(does_type_checking(&context, false));
  assert!(!does_type_checking(&context, false));
  assert!(does_type_checking(&context, true));
  assert!(does_type_checking(&context, true));
  assert!(!does_type_checking(&context, false));

  fn does_type_checking(context: &TestContext, reload: bool) -> bool {
    let mut args = vec![
      "check".to_string(),
      "check/cache_config_on_off/main.ts".to_string(),
    ];
    if reload {
      let mut slice = vec!["--reload".to_string()];
      args.append(&mut slice);
    }
    let output = context.new_command().args_vec(args).split_output().run();
    output.assert_exit_code(0);

    let stderr = output.stderr();
    stderr.contains("Check")
  }
}

#[test]
fn typecheck_declarations_ns() {
  let context = TestContextBuilder::for_jsr().build();
  let args = vec![
    "test".to_string(),
    "--doc".to_string(),
    util::root_path()
      .join("cli/tsc/dts/lib.deno.ns.d.ts")
      .to_string_lossy()
      .into_owned(),
  ];
  let output = context
    .new_command()
    .args_vec(args)
    .envs(util::env_vars_for_jsr_tests())
    .split_output()
    .run();

  println!("stdout: {}", output.stdout());
  println!("stderr: {}", output.stderr());
  output.assert_exit_code(0);
}

#[test]
fn typecheck_declarations_unstable() {
  let context = TestContext::default();
  let args = vec![
    "test".to_string(),
    "--doc".to_string(),
    "--unstable".to_string(),
    util::root_path()
      .join("cli/tsc/dts/lib.deno.unstable.d.ts")
      .to_string_lossy()
      .into_owned(),
  ];
  let output = context.new_command().args_vec(args).split_output().run();

  println!("stdout: {}", output.stdout());
  println!("stderr: {}", output.stderr());
  output.assert_exit_code(0);
}

#[test]
fn ts_no_recheck_on_redirect() {
  let test_context = TestContext::default();
  let check_command = test_context.new_command().args_vec([
    "run",
    "--check",
    "run/017_import_redirect.ts",
  ]);

  // run once
  let output = check_command.run();
  output.assert_matches_text("[WILDCARD]Check file://[WILDCARD]");

  // run again
  let output = check_command.run();
  output.assert_matches_text("Hello\n");
}

itest!(check_dts {
  args: "check --quiet check/dts/check_dts.d.ts",
  output: "check/dts/check_dts.out",
  exit_code: 1,
});

itest!(package_json_basic {
  args: "check main.ts",
  output: "package_json/basic/main.check.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  cwd: Some("package_json/basic"),
  copy_temp_dir: Some("package_json/basic"),
  exit_code: 0,
});

itest!(package_json_fail_check {
  args: "check --quiet fail_check.ts",
  output: "package_json/basic/fail_check.check.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  cwd: Some("package_json/basic"),
  copy_temp_dir: Some("package_json/basic"),
  exit_code: 1,
});

itest!(package_json_with_deno_json {
  args: "check --quiet main.ts",
  output: "package_json/deno_json/main.check.out",
  cwd: Some("package_json/deno_json/"),
  copy_temp_dir: Some("package_json/deno_json/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 1,
});

#[test]
fn check_error_in_dep_then_fix() {
  let test_context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  let correct_code =
    "export function greet(name: string) {\n  return `Hello ${name}`;\n}\n";
  let incorrect_code =
    "export function greet(name: number) {\n  return `Hello ${name}`;\n}\n";

  temp_dir.write(
    "main.ts",
    "import { greet } from './greet.ts';\n\nconsole.log(greet('world'));\n",
  );
  temp_dir.write("greet.ts", incorrect_code);

  let check_command = test_context.new_command().args_vec(["check", "main.ts"]);

  let output = check_command.run();
  output.assert_matches_text("Check [WILDCARD]main.ts\nerror: TS234[WILDCARD]");
  output.assert_exit_code(1);

  temp_dir.write("greet.ts", correct_code);
  let output = check_command.run();
  output.assert_matches_text("Check [WILDCARD]main.ts\n");

  temp_dir.write("greet.ts", incorrect_code);
  let output = check_command.run();
  output.assert_matches_text("Check [WILDCARD]main.ts\nerror: TS234[WILDCARD]");
  output.assert_exit_code(1);
}

#[test]
fn json_module_check_then_error() {
  let test_context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = test_context.temp_dir();
  let correct_code = "{ \"foo\": \"bar\" }";
  let incorrect_code = "{ \"foo2\": \"bar\" }";

  temp_dir.write(
    "main.ts",
    "import test from './test.json' assert { type: 'json' }; console.log(test.foo);\n",
  );
  temp_dir.write("test.json", correct_code);

  let check_command = test_context.new_command().args_vec(["check", "main.ts"]);

  check_command.run().assert_exit_code(0).skip_output_check();

  temp_dir.write("test.json", incorrect_code);
  check_command
    .run()
    .assert_matches_text("Check [WILDCARD]main.ts\nerror: TS2551[WILDCARD]")
    .assert_exit_code(1);
}

#[test]
fn npm_module_check_then_error() {
  let test_context = TestContextBuilder::new()
    .use_temp_cwd()
    .add_npm_env_vars()
    .use_http_server()
    .build();
  let temp_dir = test_context.temp_dir();
  temp_dir.write("deno.json", "{}"); // so the lockfile gets loaded

  // get the lockfiles values first (this is necessary because the test
  // server generates different tarballs based on the operating system)
  test_context
    .new_command()
    .args_vec([
      "cache",
      "npm:@denotest/breaking-change-between-versions@1.0.0",
      "npm:@denotest/breaking-change-between-versions@2.0.0",
    ])
    .run()
    .skip_output_check();
  let lockfile = temp_dir.path().join("deno.lock");
  let mut lockfile_content =
    lockfile.read_json::<deno_lockfile::LockfileContent>();

  // make the specifier resolve to version 1
  lockfile_content.packages.specifiers.insert(
    "npm:@denotest/breaking-change-between-versions".to_string(),
    "npm:@denotest/breaking-change-between-versions@1.0.0".to_string(),
  );
  lockfile.write_json(&lockfile_content);
  temp_dir.write(
    "main.ts",
    "import { oldName } from 'npm:@denotest/breaking-change-between-versions'; console.log(oldName());\n",
  );

  let check_command = test_context.new_command().args_vec(["check", "main.ts"]);
  check_command.run().assert_exit_code(0).skip_output_check();

  // now update the lockfile to use version 2 instead, which should cause a
  // type checking error because the oldName no longer exists
  lockfile_content.packages.specifiers.insert(
    "npm:@denotest/breaking-change-between-versions".to_string(),
    "npm:@denotest/breaking-change-between-versions@2.0.0".to_string(),
  );
  lockfile.write_json(&lockfile_content);

  check_command
    .run()
    .assert_matches_text("Check [WILDCARD]main.ts\nerror: TS2305[WILDCARD]has no exported member 'oldName'[WILDCARD]")
    .assert_exit_code(1);
}
