// Copyright 2018-2025 the Deno authors. MIT license.

use deno_lockfile::NewLockfileOptions;
use deno_lockfile::NpmPackageInfoProvider;
use deno_semver::jsr::JsrDepPackageReq;
use test_util as util;
use util::TestContext;
use util::TestContextBuilder;

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
    "check".to_string(),
    "--doc-only".to_string(),
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
    "check".to_string(),
    "--doc-only".to_string(),
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
    "--allow-import",
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
  output.assert_matches_text("Check [WILDCARD]main.ts\nTS234[WILDCARD]");
  output.assert_exit_code(1);

  temp_dir.write("greet.ts", correct_code);
  let output = check_command.run();
  output.assert_matches_text("Check [WILDCARD]main.ts\n");

  temp_dir.write("greet.ts", incorrect_code);
  let output = check_command.run();
  output.assert_matches_text("Check [WILDCARD]main.ts\nTS234[WILDCARD]");
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
    .assert_matches_text("Check [WILDCARD]main.ts\nTS2551[WILDCARD]")
    .assert_exit_code(1);
}
struct TestNpmPackageInfoProvider;

#[async_trait::async_trait(?Send)]
impl NpmPackageInfoProvider for TestNpmPackageInfoProvider {
  async fn get_npm_package_info(
    &self,
    values: &[deno_semver::package::PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    Ok(values.iter().map(|_| Default::default()).collect())
  }
}

#[tokio::test]
async fn npm_module_check_then_error() {
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
  let lockfile_path = temp_dir.path().join("deno.lock");
  let mut lockfile = deno_lockfile::Lockfile::new(
    NewLockfileOptions {
      file_path: lockfile_path.to_path_buf(),
      content: &lockfile_path.read_to_string(),
      overwrite: false,
    },
    &TestNpmPackageInfoProvider,
  )
  .await
  .unwrap();

  // make the specifier resolve to version 1
  lockfile.content.packages.specifiers.insert(
    JsrDepPackageReq::from_str(
      "npm:@denotest/breaking-change-between-versions",
    )
    .unwrap(),
    "1.0.0".into(),
  );
  lockfile_path.write(lockfile.as_json_string());
  temp_dir.write(
    "main.ts",
    "import { oldName } from 'npm:@denotest/breaking-change-between-versions'; console.log(oldName());\n",
  );

  let check_command = test_context.new_command().args_vec(["check", "main.ts"]);
  check_command.run().assert_exit_code(0).skip_output_check();

  // now update the lockfile to use version 2 instead, which should cause a
  // type checking error because the oldName no longer exists
  lockfile.content.packages.specifiers.insert(
    JsrDepPackageReq::from_str(
      "npm:@denotest/breaking-change-between-versions",
    )
    .unwrap(),
    "2.0.0".into(),
  );
  lockfile_path.write(lockfile.as_json_string());

  check_command
    .run()
    .assert_matches_text("Check [WILDCARD]main.ts\nTS2305[WILDCARD]has no exported member 'oldName'[WILDCARD]")
    .assert_exit_code(1);
}
