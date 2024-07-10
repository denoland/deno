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

#[test]
fn test_unstable_sloppy_imports_dts_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("a.ts", "export class A {}"); // resolves this
  temp_dir.write("a.d.ts", "export class A2 {}");

  temp_dir.write("b.js", "export class B {}");
  temp_dir.write("b.d.ts", "export class B2 {}"); // this

  temp_dir.write("c.mts", "export class C {}"); // this
  temp_dir.write("c.d.mts", "export class C2 {}");

  temp_dir.write("d.mjs", "export class D {}");
  temp_dir.write("d.d.mts", "export class D2 {}"); // this

  let temp_dir = temp_dir.path();

  let dir = temp_dir.join("dir_ts");
  dir.create_dir_all();
  dir.join("index.ts").write("export class Dir {}"); // this
  dir.join("index.d.ts").write("export class Dir2 {}");

  let dir = temp_dir.join("dir_js");
  dir.create_dir_all();
  dir.join("index.js").write("export class Dir {}");
  dir.join("index.d.ts").write("export class Dir2 {}"); // this

  let dir = temp_dir.join("dir_mts");
  dir.create_dir_all();
  dir.join("index.mts").write("export class Dir {}"); // this
  dir.join("index.d.ts").write("export class Dir2 {}");

  let dir = temp_dir.join("dir_mjs");
  dir.create_dir_all();
  dir.join("index.mjs").write("export class Dir {}");
  dir.join("index.d.ts").write("export class Dir2 {}"); // this

  temp_dir.join("main.ts").write(
    r#"import * as a from "./a.js";
import * as b from "./b.js";
import * as c from "./c.mjs";
import * as d from "./d.mjs";

console.log(a.A);
console.log(b.B2);
console.log(c.C);
console.log(d.D2);

import * as a2 from "./a";
import * as b2 from "./b";
import * as c2 from "./c";
import * as d2 from "./d";

console.log(a2.A);
console.log(b2.B2);
console.log(c2.C);
console.log(d2.D2);

import * as dirTs from "./dir_ts";
import * as dirJs from "./dir_js";
import * as dirMts from "./dir_mts";
import * as dirMjs from "./dir_mjs";

console.log(dirTs.Dir);
console.log(dirJs.Dir2);
console.log(dirMts.Dir);
console.log(dirMjs.Dir2);
"#,
  );

  context
    .new_command()
    .args("check --unstable-sloppy-imports main.ts")
    .run()
    .assert_matches_text(
      r#"Warning Sloppy module resolution (hint: update .js extension to .ts)
    at file:///[WILDCARD]/main.ts:1:20
Warning Sloppy module resolution (hint: update .mjs extension to .mts)
    at file:///[WILDCARD]/main.ts:3:20
Warning Sloppy module resolution (hint: add .ts extension)
    at file:///[WILDCARD]/main.ts:11:21
Warning Sloppy module resolution (hint: add .js extension)
    at file:///[WILDCARD]/main.ts:12:21
Warning Sloppy module resolution (hint: add .mts extension)
    at file:///[WILDCARD]/main.ts:13:21
Warning Sloppy module resolution (hint: add .mjs extension)
    at file:///[WILDCARD]/main.ts:14:21
Warning Sloppy module resolution (hint: specify path to index.ts file in directory instead)
    at file:///[WILDCARD]/main.ts:21:24
Warning Sloppy module resolution (hint: specify path to index.js file in directory instead)
    at file:///[WILDCARD]/main.ts:22:24
Warning Sloppy module resolution (hint: specify path to index.mts file in directory instead)
    at file:///[WILDCARD]/main.ts:23:25
Warning Sloppy module resolution (hint: specify path to index.mjs file in directory instead)
    at file:///[WILDCARD]/main.ts:24:25
Check [WILDCARD]main.ts
"#,
    )
    .assert_exit_code(0);
}
