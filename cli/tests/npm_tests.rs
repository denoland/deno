// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod integration;

mod npm {
  use super::*;
  use pretty_assertions::assert_eq;
  use std::process::Stdio;
  use test_util as util;
  use util::assert_contains;
  use util::env_vars_for_npm_tests;
  use util::env_vars_for_npm_tests_no_sync_download;
  use util::http_server;

  // NOTE: See how to make test npm packages at ./testdata/npm/README.md

  itest!(esm_module {
    args: "run --allow-read --allow-env npm/esm/main.js",
    output: "npm/esm/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(esm_module_eval {
  args_vec: vec![
    "eval",
    "import chalk from 'npm:chalk@5'; console.log(chalk.green('chalk esm loads'));",
  ],
  output: "npm/esm/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

  itest!(esm_module_deno_test {
    args: "test --allow-read --allow-env --unstable npm/esm/test.js",
    output: "npm/esm/test.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(esm_import_cjs_default {
  args: "run --allow-read --allow-env --unstable --quiet --check=all npm/esm_import_cjs_default/main.ts",
  output: "npm/esm_import_cjs_default/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

  itest!(cjs_with_deps {
    args: "run --allow-read --allow-env npm/cjs_with_deps/main.js",
    output: "npm/cjs_with_deps/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(cjs_sub_path {
    args: "run --allow-read npm/cjs_sub_path/main.js",
    output: "npm/cjs_sub_path/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(cjs_local_global_decls {
    args: "run --allow-read npm/cjs_local_global_decls/main.ts",
    output: "npm/cjs_local_global_decls/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(cjs_reexport_collision {
    args: "run -A --quiet npm/cjs_reexport_collision/main.ts",
    output: "npm/cjs_reexport_collision/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(cjs_this_in_exports {
    args: "run --allow-read --quiet npm/cjs_this_in_exports/main.js",
    output: "npm/cjs_this_in_exports/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(translate_cjs_to_esm {
    args: "run -A --quiet npm/translate_cjs_to_esm/main.js",
    output: "npm/translate_cjs_to_esm/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(compare_globals {
    args: "run --allow-read --unstable --check=all npm/compare_globals/main.ts",
    output: "npm/compare_globals/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(conditional_exports {
    args: "run --allow-read npm/conditional_exports/main.js",
    output: "npm/conditional_exports/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(conditional_exports_node_modules_dir {
    args:
      "run --allow-read --node-modules-dir $TESTDATA/npm/conditional_exports/main.js",
    output: "npm/conditional_exports/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    temp_cwd: true,
  });

  itest!(dual_cjs_esm {
    args: "run -A --quiet npm/dual_cjs_esm/main.ts",
    output: "npm/dual_cjs_esm/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(child_process_fork_test {
    args: "run -A --quiet npm/child_process_fork_test/main.ts",
    output: "npm/child_process_fork_test/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(cjs_module_export_assignment {
  args: "run -A --unstable --quiet --check=all npm/cjs_module_export_assignment/main.ts",
  output: "npm/cjs_module_export_assignment/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

  itest!(cjs_module_export_assignment_number {
  args: "run -A --unstable --quiet --check=all npm/cjs_module_export_assignment_number/main.ts",
  output: "npm/cjs_module_export_assignment_number/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

  itest!(mixed_case_package_name_global_dir {
    args: "run npm/mixed_case_package_name/global.ts",
    output: "npm/mixed_case_package_name/global.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(mixed_case_package_name_local_dir {
    args:
      "run --node-modules-dir -A $TESTDATA/npm/mixed_case_package_name/local.ts",
    output: "npm/mixed_case_package_name/local.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
    temp_cwd: true,
  });

  // FIXME(bartlomieju): npm: specifiers are not handled in dynamic imports
  // at the moment
  // itest!(dynamic_import {
  //   args: "run --allow-read --allow-env npm/dynamic_import/main.ts",
  //   output: "npm/dynamic_import/main.out",
  //   envs: env_vars_for_npm_tests(),
  //   http_server: true,
  // });

  itest!(dynamic_import_reload_same_package {
    args: "run -A --reload npm/dynamic_import_reload_same_package/main.ts",
    output: "npm/dynamic_import_reload_same_package/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(env_var_re_export_dev {
    args: "run --allow-read --allow-env --quiet npm/env_var_re_export/main.js",
    output_str: Some("dev\n"),
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(env_var_re_export_prod {
    args: "run --allow-read --allow-env --quiet npm/env_var_re_export/main.js",
    output_str: Some("prod\n"),
    envs: {
      let mut vars = env_vars_for_npm_tests();
      vars.push(("NODE_ENV".to_string(), "production".to_string()));
      vars
    },
    http_server: true,
  });

  itest!(cached_only {
    args: "run --cached-only npm/cached_only/main.ts",
    output: "npm/cached_only/main.out",
    envs: env_vars_for_npm_tests(),
    exit_code: 1,
  });

  itest!(import_map {
    args: "run --allow-read --allow-env --import-map npm/import_map/import_map.json npm/import_map/main.js",
    output: "npm/import_map/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(lock_file {
    args: "run --allow-read --allow-env --lock npm/lock_file/lock.json npm/lock_file/main.js",
    output: "npm/lock_file/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 10,
  });

  itest!(sub_paths {
    args: "run -A --quiet npm/sub_paths/main.jsx",
    output: "npm/sub_paths/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(remote_npm_specifier {
    args: "run --quiet npm/remote_npm_specifier/main.ts",
    output: "npm/remote_npm_specifier/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(tarball_with_global_header {
    args: "run -A --quiet npm/tarball_with_global_header/main.js",
    output: "npm/tarball_with_global_header/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(nonexistent_file {
    args: "run -A --quiet npm/nonexistent_file/main.js",
    output: "npm/nonexistent_file/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(invalid_package_name {
    args: "run -A --quiet npm/invalid_package_name/main.js",
    output: "npm/invalid_package_name/main.out",
    envs: env_vars_for_npm_tests(),
    exit_code: 1,
  });

  itest!(require_json {
    args: "run -A --quiet npm/require_json/main.js",
    output: "npm/require_json/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(error_version_after_subpath {
    args: "run -A --quiet npm/error_version_after_subpath/main.js",
    output: "npm/error_version_after_subpath/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(deno_cache {
    args: "cache --reload npm:chalk npm:mkdirp",
    output: "npm/deno_cache.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(check_all {
    args: "check --all npm/check_errors/main.ts",
    output: "npm/check_errors/main_all.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(check_local {
    args: "check npm/check_errors/main.ts",
    output: "npm/check_errors/main_local.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(types_general {
    args: "check --quiet npm/types/main.ts",
    output: "npm/types/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(types_ambient_module {
    args: "check --quiet npm/types_ambient_module/main.ts",
    output: "npm/types_ambient_module/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(types_ambient_module_import_map {
    args: "check --quiet --import-map=npm/types_ambient_module/import_map.json npm/types_ambient_module/main_import_map.ts",
    output: "npm/types_ambient_module/main_import_map.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(no_types_cjs {
    args: "check --quiet npm/no_types_cjs/main.ts",
    output_str: Some(""),
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(no_types_in_conditional_exports {
    args: "run --check --unstable npm/no_types_in_conditional_exports/main.ts",
    output: "npm/no_types_in_conditional_exports/main.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(types_entry_value_not_exists {
    args: "check --all npm/types_entry_value_not_exists/main.ts",
    output: "npm/types_entry_value_not_exists/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(types_exports_import_types {
    args: "check --all npm/types_exports_import_types/main.ts",
    output: "npm/types_exports_import_types/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(types_no_types_entry {
    args: "check --all npm/types_no_types_entry/main.ts",
    output: "npm/types_no_types_entry/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(typescript_file_in_package {
    args: "run npm/typescript_file_in_package/main.ts",
    output: "npm/typescript_file_in_package/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  #[test]
  fn parallel_downloading() {
    let (out, _err) = util::run_and_collect_output_with_args(
      true,
      vec![
        "run",
        "--allow-read",
        "--allow-env",
        "npm/cjs_with_deps/main.js",
      ],
      None,
      // don't use the sync env var
      Some(env_vars_for_npm_tests_no_sync_download()),
      true,
    );
    assert!(out.contains("chalk cjs loads"));
  }

  #[test]
  fn cached_only_after_first_run() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("npm/cached_only_after_first_run/main1.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--cached-only")
      .arg("npm/cached_only_after_first_run/main2.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(
    stderr,
    "An npm specifier not found in cache: \"ansi-styles\", --cached-only is specified."
  );
    assert!(stdout.is_empty());
    assert!(!output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--cached-only")
      .arg("npm/cached_only_after_first_run/main1.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();

    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stderr.is_empty());
    assert_contains!(stdout, "createChalk: chalk");
  }

  #[test]
  fn reload_flag() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("npm/reload/main.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--reload")
      .arg("npm/reload/main.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--reload=npm:")
      .arg("npm/reload/main.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--reload=npm:chalk")
      .arg("npm/reload/main.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--reload=npm:foobar")
      .arg("npm/reload/main.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stderr.is_empty());
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());
  }

  #[test]
  fn no_npm_after_first_run() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--no-npm")
      .arg("npm/no_npm_after_first_run/main1.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(
    stderr,
    "Following npm specifiers were requested: \"chalk@5\"; but --no-npm is specified."
  );
    assert!(stdout.is_empty());
    assert!(!output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("npm/no_npm_after_first_run/main1.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(stderr, "Download");
    assert_contains!(stdout, "createChalk: chalk");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--no-npm")
      .arg("npm/no_npm_after_first_run/main1.ts")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_contains!(
    stderr,
    "Following npm specifiers were requested: \"chalk@5\"; but --no-npm is specified."
  );
    assert!(stdout.is_empty());
    assert!(!output.status.success());
  }

  #[test]
  fn deno_run_cjs_module() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(deno_dir.path())
      .arg("run")
      .arg("--allow-read")
      .arg("--allow-env")
      .arg("--allow-write")
      .arg("npm:mkdirp@1.0.4")
      .arg("test_dir")
      .env("NO_COLOR", "1")
      .envs(env_vars_for_npm_tests())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());

    assert!(deno_dir.path().join("test_dir").exists());
  }

  itest!(deno_run_cowsay {
    args: "run -A --quiet npm:cowsay@1.5.0 Hello",
    output: "npm/deno_run_cowsay.out",
    envs: env_vars_for_npm_tests_no_sync_download(),
    http_server: true,
  });

  itest!(deno_run_cowsay_explicit {
    args: "run -A --quiet npm:cowsay@1.5.0/cowsay Hello",
    output: "npm/deno_run_cowsay.out",
    envs: env_vars_for_npm_tests_no_sync_download(),
    http_server: true,
  });

  itest!(deno_run_cowthink {
    args: "run -A --quiet npm:cowsay@1.5.0/cowthink Hello",
    output: "npm/deno_run_cowthink.out",
    envs: env_vars_for_npm_tests_no_sync_download(),
    http_server: true,
  });

  itest!(deno_run_bin_esm {
    args: "run -A --quiet npm:@denotest/bin/cli-esm this is a test",
    output: "npm/deno_run_esm.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(deno_run_bin_no_ext {
    args: "run -A --quiet npm:@denotest/bin/cli-no-ext this is a test",
    output: "npm/deno_run_no_ext.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(deno_run_bin_cjs {
    args: "run -A --quiet npm:@denotest/bin/cli-cjs this is a test",
    output: "npm/deno_run_cjs.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(deno_run_non_existent {
    args: "run npm:mkdirp@0.5.125",
    output: "npm/deno_run_non_existent.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  });

  itest!(builtin_module_module {
    args: "run --allow-read --quiet npm/builtin_module_module/main.js",
    output: "npm/builtin_module_module/main.out",
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(node_modules_dir_require_added_node_modules_folder {
  args:
    "run --node-modules-dir -A --quiet $TESTDATA/npm/require_added_nm_folder/main.js",
  output: "npm/require_added_nm_folder/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 0,
  temp_cwd: true,
});

  itest!(node_modules_dir_with_deps {
  args: "run --allow-read --allow-env --node-modules-dir $TESTDATA/npm/cjs_with_deps/main.js",
  output: "npm/cjs_with_deps/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  temp_cwd: true,
});

  itest!(node_modules_dir_yargs {
  args: "run --allow-read --allow-env --node-modules-dir $TESTDATA/npm/cjs_yargs/main.js",
  output: "npm/cjs_yargs/main.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
  temp_cwd: true,
});

  #[test]
  fn node_modules_dir_cache() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(deno_dir.path())
      .arg("cache")
      .arg("--node-modules-dir")
      .arg("--quiet")
      .arg(util::testdata_path().join("npm/dual_cjs_esm/main.ts"))
      .envs(env_vars_for_npm_tests())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());

    let node_modules = deno_dir.path().join("node_modules");
    assert!(node_modules
    .join(
      ".deno/@denotest+dual-cjs-esm@1.0.0/node_modules/@denotest/dual-cjs-esm"
    )
    .exists());
    assert!(node_modules.join("@denotest/dual-cjs-esm").exists());

    // now try deleting the folder with the package source in the npm cache dir
    let package_global_cache_dir = deno_dir
      .path()
      .join("npm")
      .join("localhost_4545")
      .join("npm")
      .join("registry")
      .join("@denotest")
      .join("dual-cjs-esm")
      .join("1.0.0");
    assert!(package_global_cache_dir.exists());
    std::fs::remove_dir_all(&package_global_cache_dir).unwrap();

    // run the output, and it shouldn't bother recreating the directory
    // because it already has everything cached locally in the node_modules folder
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(deno_dir.path())
      .arg("run")
      .arg("--node-modules-dir")
      .arg("--quiet")
      .arg("-A")
      .arg(util::testdata_path().join("npm/dual_cjs_esm/main.ts"))
      .envs(env_vars_for_npm_tests())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());

    // this won't exist, but actually the parent directory
    // will because it still re-downloads the registry information
    assert!(!package_global_cache_dir.exists());
  }

  #[test]
  fn ensure_registry_files_local() {
    // ensures the registry files all point at local tarballs
    let registry_dir_path = util::testdata_path().join("npm").join("registry");
    for entry in std::fs::read_dir(&registry_dir_path).unwrap() {
      let entry = entry.unwrap();
      if entry.metadata().unwrap().is_dir() {
        let registry_json_path = registry_dir_path
          .join(entry.file_name())
          .join("registry.json");
        if registry_json_path.exists() {
          let file_text = std::fs::read_to_string(&registry_json_path).unwrap();
          if file_text.contains("https://registry.npmjs.org/") {
            panic!(
              "file {} contained a reference to the npm registry",
              registry_json_path.display(),
            );
          }
        }
      }
    }
  }

  itest!(compile_errors {
    args: "compile -A --quiet npm/cached_only/main.ts",
    output_str: Some("error: npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: npm:chalk@5\n"),
    exit_code: 1,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(bundle_errors {
    args: "bundle --quiet npm/esm/main.js",
    output_str: Some("error: npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: npm:chalk@5\n"),
    exit_code: 1,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(info_chalk_display {
    args: "info --quiet npm/cjs_with_deps/main.js",
    output: "npm/cjs_with_deps/main_info.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(info_chalk_display_node_modules_dir {
    args: "info --quiet --node-modules-dir $TESTDATA/npm/cjs_with_deps/main.js",
    output: "npm/cjs_with_deps/main_info.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
    temp_cwd: true,
  });

  itest!(info_chalk_json {
    args: "info --quiet --json npm/cjs_with_deps/main.js",
    output: "npm/cjs_with_deps/main_info_json.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(info_chalk_json_node_modules_dir {
    args:
      "info --quiet --node-modules-dir --json $TESTDATA/npm/cjs_with_deps/main.js",
    output: "npm/cjs_with_deps/main_info_json.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
    temp_cwd: true,
  });

  itest!(info_cli_chalk_display {
    args: "info --quiet npm:chalk@4",
    output: "npm/info/chalk.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(info_cli_chalk_json {
    args: "info --quiet --json npm:chalk@4",
    output: "npm/info/chalk_json.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  #[test]
  fn lock_file_missing_top_level_package() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();
    let temp_dir = util::TempDir::new();

    // write empty config file
    temp_dir.write("deno.json", "{}");

    // Lock file that is automatically picked up has been intentionally broken,
    // by removing "cowsay" package from it. This test ensures that npm resolver
    // snapshot can be successfully hydrated in such situation
    let lock_file_content = r#"{
    "version": "2",
    "remote": {},
    "npm": {
      "specifiers": { "cowsay": "cowsay@1.5.0" },
      "packages": {
        "ansi-regex@3.0.1": {
          "integrity": "sha512-+O9Jct8wf++lXxxFc4hc8LsjaSq0HFzzL7cVsw8pRDIPdjKD2mT4ytDZlLuSBZ4cLKZFXIrMGO7DbQCtMJJMKw==",
          "dependencies": {}
        },
        "ansi-regex@5.0.1": {
          "integrity": "sha512-quJQXlTSUGL2LH9SUXo8VwsY4soanhgo6LNSm84E1LBcE8s3O0wpdiRzyR9z/ZZJMlMWv37qOOb9pdJlMUEKFQ==",
          "dependencies": {}
        },
        "ansi-styles@4.3.0": {
          "integrity": "sha512-zbB9rCJAT1rbjiVDb2hqKFHNYLxgtk8NURxZ3IZwD3F6NtxbXZQCnnSi1Lkx+IDohdPlFp222wVALIheZJQSEg==",
          "dependencies": { "color-convert": "color-convert@2.0.1" }
        },
        "camelcase@5.3.1": {
          "integrity": "sha512-L28STB170nwWS63UjtlEOE3dldQApaJXZkOI1uMFfzf3rRuPegHaHesyee+YxQ+W6SvRDQV6UrdOdRiR153wJg==",
          "dependencies": {}
        },
        "cliui@6.0.0": {
          "integrity": "sha512-t6wbgtoCXvAzst7QgXxJYqPt0usEfbgQdftEPbLL/cvv6HPE5VgvqCuAIDR0NgU52ds6rFwqrgakNLrHEjCbrQ==",
          "dependencies": {
            "string-width": "string-width@4.2.3",
            "strip-ansi": "strip-ansi@6.0.1",
            "wrap-ansi": "wrap-ansi@6.2.0"
          }
        },
        "color-convert@2.0.1": {
          "integrity": "sha512-RRECPsj7iu/xb5oKYcsFHSppFNnsj/52OVTRKb4zP5onXwVF3zVmmToNcOfGC+CRDpfK/U584fMg38ZHCaElKQ==",
          "dependencies": { "color-name": "color-name@1.1.4" }
        },
        "color-name@1.1.4": {
          "integrity": "sha512-dOy+3AuW3a2wNbZHIuMZpTcgjGuLU/uBL/ubcZF9OXbDo8ff4O8yVp5Bf0efS8uEoYo5q4Fx7dY9OgQGXgAsQA==",
          "dependencies": {}
        },
        "decamelize@1.2.0": {
          "integrity": "sha512-z2S+W9X73hAUUki+N+9Za2lBlun89zigOyGrsax+KUQ6wKW4ZoWpEYBkGhQjwAjjDCkWxhY0VKEhk8wzY7F5cA==",
          "dependencies": {}
        },
        "emoji-regex@8.0.0": {
          "integrity": "sha512-MSjYzcWNOA0ewAHpz0MxpYFvwg6yjy1NG3xteoqz644VCo/RPgnr1/GGt+ic3iJTzQ8Eu3TdM14SawnVUmGE6A==",
          "dependencies": {}
        },
        "find-up@4.1.0": {
          "integrity": "sha512-PpOwAdQ/YlXQ2vj8a3h8IipDuYRi3wceVQQGYWxNINccq40Anw7BlsEXCMbt1Zt+OLA6Fq9suIpIWD0OsnISlw==",
          "dependencies": {
            "locate-path": "locate-path@5.0.0",
            "path-exists": "path-exists@4.0.0"
          }
        },
        "get-caller-file@2.0.5": {
          "integrity": "sha512-DyFP3BM/3YHTQOCUL/w0OZHR0lpKeGrxotcHWcqNEdnltqFwXVfhEBQ94eIo34AfQpo0rGki4cyIiftY06h2Fg==",
          "dependencies": {}
        },
        "get-stdin@8.0.0": {
          "integrity": "sha512-sY22aA6xchAzprjyqmSEQv4UbAAzRN0L2dQB0NlN5acTTK9Don6nhoc3eAbUnpZiCANAMfd/+40kVdKfFygohg==",
          "dependencies": {}
        },
        "is-fullwidth-code-point@2.0.0": {
          "integrity": "sha512-VHskAKYM8RfSFXwee5t5cbN5PZeq1Wrh6qd5bkyiXIf6UQcN6w/A0eXM9r6t8d+GYOh+o6ZhiEnb88LN/Y8m2w==",
          "dependencies": {}
        },
        "is-fullwidth-code-point@3.0.0": {
          "integrity": "sha512-zymm5+u+sCsSWyD9qNaejV3DFvhCKclKdizYaJUuHA83RLjb7nSuGnddCHGv0hk+KY7BMAlsWeK4Ueg6EV6XQg==",
          "dependencies": {}
        },
        "locate-path@5.0.0": {
          "integrity": "sha512-t7hw9pI+WvuwNJXwk5zVHpyhIqzg2qTlklJOf0mVxGSbe3Fp2VieZcduNYjaLDoy6p9uGpQEGWG87WpMKlNq8g==",
          "dependencies": { "p-locate": "p-locate@4.1.0" }
        },
        "p-limit@2.3.0": {
          "integrity": "sha512-//88mFWSJx8lxCzwdAABTJL2MyWB12+eIY7MDL2SqLmAkeKU9qxRvWuSyTjm3FUmpBEMuFfckAIqEaVGUDxb6w==",
          "dependencies": { "p-try": "p-try@2.2.0" }
        },
        "p-locate@4.1.0": {
          "integrity": "sha512-R79ZZ/0wAxKGu3oYMlz8jy/kbhsNrS7SKZ7PxEHBgJ5+F2mtFW2fK2cOtBh1cHYkQsbzFV7I+EoRKe6Yt0oK7A==",
          "dependencies": { "p-limit": "p-limit@2.3.0" }
        },
        "p-try@2.2.0": {
          "integrity": "sha512-R4nPAVTAU0B9D35/Gk3uJf/7XYbQcyohSKdvAxIRSNghFl4e71hVoGnBNQz9cWaXxO2I10KTC+3jMdvvoKw6dQ==",
          "dependencies": {}
        },
        "path-exists@4.0.0": {
          "integrity": "sha512-ak9Qy5Q7jYb2Wwcey5Fpvg2KoAc/ZIhLSLOSBmRmygPsGwkVVt0fZa0qrtMz+m6tJTAHfZQ8FnmB4MG4LWy7/w==",
          "dependencies": {}
        },
        "require-directory@2.1.1": {
          "integrity": "sha512-fGxEI7+wsG9xrvdjsrlmL22OMTTiHRwAMroiEeMgq8gzoLC/PQr7RsRDSTLUg/bZAZtF+TVIkHc6/4RIKrui+Q==",
          "dependencies": {}
        },
        "require-main-filename@2.0.0": {
          "integrity": "sha512-NKN5kMDylKuldxYLSUfrbo5Tuzh4hd+2E8NPPX02mZtn1VuREQToYe/ZdlJy+J3uCpfaiGF05e7B8W0iXbQHmg==",
          "dependencies": {}
        },
        "set-blocking@2.0.0": {
          "integrity": "sha512-KiKBS8AnWGEyLzofFfmvKwpdPzqiy16LvQfK3yv/fVH7Bj13/wl3JSR1J+rfgRE9q7xUJK4qvgS8raSOeLUehw==",
          "dependencies": {}
        },
        "string-width@2.1.1": {
          "integrity": "sha512-nOqH59deCq9SRHlxq1Aw85Jnt4w6KvLKqWVik6oA9ZklXLNIOlqg4F2yrT1MVaTjAqvVwdfeZ7w7aCvJD7ugkw==",
          "dependencies": {
            "is-fullwidth-code-point": "is-fullwidth-code-point@2.0.0",
            "strip-ansi": "strip-ansi@4.0.0"
          }
        },
        "string-width@4.2.3": {
          "integrity": "sha512-wKyQRQpjJ0sIp62ErSZdGsjMJWsap5oRNihHhu6G7JVO/9jIB6UyevL+tXuOqrng8j/cxKTWyWUwvSTriiZz/g==",
          "dependencies": {
            "emoji-regex": "emoji-regex@8.0.0",
            "is-fullwidth-code-point": "is-fullwidth-code-point@3.0.0",
            "strip-ansi": "strip-ansi@6.0.1"
          }
        },
        "strip-ansi@4.0.0": {
          "integrity": "sha512-4XaJ2zQdCzROZDivEVIDPkcQn8LMFSa8kj8Gxb/Lnwzv9A8VctNZ+lfivC/sV3ivW8ElJTERXZoPBRrZKkNKow==",
          "dependencies": { "ansi-regex": "ansi-regex@3.0.1" }
        },
        "strip-ansi@6.0.1": {
          "integrity": "sha512-Y38VPSHcqkFrCpFnQ9vuSXmquuv5oXOKpGeT6aGrr3o3Gc9AlVa6JBfUSOCnbxGGZF+/0ooI7KrPuUSztUdU5A==",
          "dependencies": { "ansi-regex": "ansi-regex@5.0.1" }
        },
        "strip-final-newline@2.0.0": {
          "integrity": "sha512-BrpvfNAE3dcvq7ll3xVumzjKjZQ5tI1sEUIKr3Uoks0XUl45St3FlatVqef9prk4jRDzhW6WZg+3bk93y6pLjA==",
          "dependencies": {}
        },
        "which-module@2.0.0": {
          "integrity": "sha512-B+enWhmw6cjfVC7kS8Pj9pCrKSc5txArRyaYGe088shv/FGWH+0Rjx/xPgtsWfsUtS27FkP697E4DDhgrgoc0Q==",
          "dependencies": {}
        },
        "wrap-ansi@6.2.0": {
          "integrity": "sha512-r6lPcBGxZXlIcymEu7InxDMhdW0KDxpLgoFLcguasxCaJ/SOIZwINatK9KY/tf+ZrlywOKU0UDj3ATXUBfxJXA==",
          "dependencies": {
            "ansi-styles": "ansi-styles@4.3.0",
            "string-width": "string-width@4.2.3",
            "strip-ansi": "strip-ansi@6.0.1"
          }
        },
        "y18n@4.0.3": {
          "integrity": "sha512-JKhqTOwSrqNA1NY5lSztJ1GrBiUodLMmIZuLiDaMRJ+itFd+ABVE8XBjOvIWL+rSqNDC74LCSFmlb/U4UZ4hJQ==",
          "dependencies": {}
        },
        "yargs-parser@18.1.3": {
          "integrity": "sha512-o50j0JeToy/4K6OZcaQmW6lyXXKhq7csREXcDwk2omFPJEwUNOVtJKvmDr9EI1fAJZUyZcRF7kxGBWmRXudrCQ==",
          "dependencies": {
            "camelcase": "camelcase@5.3.1",
            "decamelize": "decamelize@1.2.0"
          }
        },
        "yargs@15.4.1": {
          "integrity": "sha512-aePbxDmcYW++PaqBsJ+HYUFwCdv4LVvdnhBy78E57PIor8/OVvhMrADFFEDh8DHDFRv/O9i3lPhsENjO7QX0+A==",
          "dependencies": {
            "cliui": "cliui@6.0.0",
            "decamelize": "decamelize@1.2.0",
            "find-up": "find-up@4.1.0",
            "get-caller-file": "get-caller-file@2.0.5",
            "require-directory": "require-directory@2.1.1",
            "require-main-filename": "require-main-filename@2.0.0",
            "set-blocking": "set-blocking@2.0.0",
            "string-width": "string-width@4.2.3",
            "which-module": "which-module@2.0.0",
            "y18n": "y18n@4.0.3",
            "yargs-parser": "yargs-parser@18.1.3"
          }
        }
      }
    }
  }
  "#;
    temp_dir.write("deno.lock", lock_file_content);
    let main_contents = r#"
  import cowsay from "npm:cowsay";
  console.log(cowsay);
  "#;
    temp_dir.write("main.ts", main_contents);

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--quiet")
      .arg("--lock")
      .arg("deno.lock")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
    stderr,
    "error: failed reading lockfile 'deno.lock'\n\nCaused by:\n    the lockfile is corrupt. You can recreate it with --lock-write\n"
  );
  }

  #[test]
  fn lock_file_lock_write() {
    // https://github.com/denoland/deno/issues/16666
    // Ensure that --lock-write still adds npm packages to the lockfile
    let _server = http_server();

    let deno_dir = util::new_deno_dir();
    let temp_dir = util::TempDir::new();

    // write empty config file
    temp_dir.write("deno.json", "{}");

    // write a lock file with borked integrity
    let lock_file_content = r#"{
  "version": "2",
  "remote": {},
  "npm": {
    "specifiers": { "cowsay@1.5.0": "cowsay@1.5.0" },
    "packages": {
      "ansi-regex@3.0.1": {
        "integrity": "sha512-+O9Jct8wf++lXxxFc4hc8LsjaSq0HFzzL7cVsw8pRDIPdjKD2mT4ytDZlLuSBZ4cLKZFXIrMGO7DbQCtMJJMKw==",
        "dependencies": {}
      },
      "ansi-regex@5.0.1": {
        "integrity": "sha512-quJQXlTSUGL2LH9SUXo8VwsY4soanhgo6LNSm84E1LBcE8s3O0wpdiRzyR9z/ZZJMlMWv37qOOb9pdJlMUEKFQ==",
        "dependencies": {}
      },
      "ansi-styles@4.3.0": {
        "integrity": "sha512-zbB9rCJAT1rbjiVDb2hqKFHNYLxgtk8NURxZ3IZwD3F6NtxbXZQCnnSi1Lkx+IDohdPlFp222wVALIheZJQSEg==",
        "dependencies": { "color-convert": "color-convert@2.0.1" }
      },
      "camelcase@5.3.1": {
        "integrity": "sha512-L28STB170nwWS63UjtlEOE3dldQApaJXZkOI1uMFfzf3rRuPegHaHesyee+YxQ+W6SvRDQV6UrdOdRiR153wJg==",
        "dependencies": {}
      },
      "cliui@6.0.0": {
        "integrity": "sha512-t6wbgtoCXvAzst7QgXxJYqPt0usEfbgQdftEPbLL/cvv6HPE5VgvqCuAIDR0NgU52ds6rFwqrgakNLrHEjCbrQ==",
        "dependencies": {
          "string-width": "string-width@4.2.3",
          "strip-ansi": "strip-ansi@6.0.1",
          "wrap-ansi": "wrap-ansi@6.2.0"
        }
      },
      "color-convert@2.0.1": {
        "integrity": "sha512-RRECPsj7iu/xb5oKYcsFHSppFNnsj/52OVTRKb4zP5onXwVF3zVmmToNcOfGC+CRDpfK/U584fMg38ZHCaElKQ==",
        "dependencies": { "color-name": "color-name@1.1.4" }
      },
      "color-name@1.1.4": {
        "integrity": "sha512-dOy+3AuW3a2wNbZHIuMZpTcgjGuLU/uBL/ubcZF9OXbDo8ff4O8yVp5Bf0efS8uEoYo5q4Fx7dY9OgQGXgAsQA==",
        "dependencies": {}
      },
      "cowsay@1.5.0": {
        "integrity": "sha512-8Ipzr54Z8zROr/62C8f0PdhQcDusS05gKTS87xxdji8VbWefWly0k8BwGK7+VqamOrkv3eGsCkPtvlHzrhWsCA==",
        "dependencies": {
          "get-stdin": "get-stdin@8.0.0",
          "string-width": "string-width@2.1.1",
          "strip-final-newline": "strip-final-newline@2.0.0",
          "yargs": "yargs@15.4.1"
        }
      },
      "decamelize@1.2.0": {
        "integrity": "sha512-z2S+W9X73hAUUki+N+9Za2lBlun89zigOyGrsax+KUQ6wKW4ZoWpEYBkGhQjwAjjDCkWxhY0VKEhk8wzY7F5cA==",
        "dependencies": {}
      },
      "emoji-regex@8.0.0": {
        "integrity": "sha512-MSjYzcWNOA0ewAHpz0MxpYFvwg6yjy1NG3xteoqz644VCo/RPgnr1/GGt+ic3iJTzQ8Eu3TdM14SawnVUmGE6A==",
        "dependencies": {}
      },
      "find-up@4.1.0": {
        "integrity": "sha512-PpOwAdQ/YlXQ2vj8a3h8IipDuYRi3wceVQQGYWxNINccq40Anw7BlsEXCMbt1Zt+OLA6Fq9suIpIWD0OsnISlw==",
        "dependencies": {
          "locate-path": "locate-path@5.0.0",
          "path-exists": "path-exists@4.0.0"
        }
      },
      "get-caller-file@2.0.5": {
        "integrity": "sha512-DyFP3BM/3YHTQOCUL/w0OZHR0lpKeGrxotcHWcqNEdnltqFwXVfhEBQ94eIo34AfQpo0rGki4cyIiftY06h2Fg==",
        "dependencies": {}
      },
      "get-stdin@8.0.0": {
        "integrity": "sha512-sY22aA6xchAzprjyqmSEQv4UbAAzRN0L2dQB0NlN5acTTK9Don6nhoc3eAbUnpZiCANAMfd/+40kVdKfFygohg==",
        "dependencies": {}
      },
      "is-fullwidth-code-point@2.0.0": {
        "integrity": "sha512-VHskAKYM8RfSFXwee5t5cbN5PZeq1Wrh6qd5bkyiXIf6UQcN6w/A0eXM9r6t8d+GYOh+o6ZhiEnb88LN/Y8m2w==",
        "dependencies": {}
      },
      "is-fullwidth-code-point@3.0.0": {
        "integrity": "sha512-zymm5+u+sCsSWyD9qNaejV3DFvhCKclKdizYaJUuHA83RLjb7nSuGnddCHGv0hk+KY7BMAlsWeK4Ueg6EV6XQg==",
        "dependencies": {}
      },
      "locate-path@5.0.0": {
        "integrity": "sha512-t7hw9pI+WvuwNJXwk5zVHpyhIqzg2qTlklJOf0mVxGSbe3Fp2VieZcduNYjaLDoy6p9uGpQEGWG87WpMKlNq8g==",
        "dependencies": { "p-locate": "p-locate@4.1.0" }
      },
      "p-limit@2.3.0": {
        "integrity": "sha512-//88mFWSJx8lxCzwdAABTJL2MyWB12+eIY7MDL2SqLmAkeKU9qxRvWuSyTjm3FUmpBEMuFfckAIqEaVGUDxb6w==",
        "dependencies": { "p-try": "p-try@2.2.0" }
      },
      "p-locate@4.1.0": {
        "integrity": "sha512-R79ZZ/0wAxKGu3oYMlz8jy/kbhsNrS7SKZ7PxEHBgJ5+F2mtFW2fK2cOtBh1cHYkQsbzFV7I+EoRKe6Yt0oK7A==",
        "dependencies": { "p-limit": "p-limit@2.3.0" }
      },
      "p-try@2.2.0": {
        "integrity": "sha512-R4nPAVTAU0B9D35/Gk3uJf/7XYbQcyohSKdvAxIRSNghFl4e71hVoGnBNQz9cWaXxO2I10KTC+3jMdvvoKw6dQ==",
        "dependencies": {}
      },
      "path-exists@4.0.0": {
        "integrity": "sha512-ak9Qy5Q7jYb2Wwcey5Fpvg2KoAc/ZIhLSLOSBmRmygPsGwkVVt0fZa0qrtMz+m6tJTAHfZQ8FnmB4MG4LWy7/w==",
        "dependencies": {}
      },
      "require-directory@2.1.1": {
        "integrity": "sha512-fGxEI7+wsG9xrvdjsrlmL22OMTTiHRwAMroiEeMgq8gzoLC/PQr7RsRDSTLUg/bZAZtF+TVIkHc6/4RIKrui+Q==",
        "dependencies": {}
      },
      "require-main-filename@2.0.0": {
        "integrity": "sha512-NKN5kMDylKuldxYLSUfrbo5Tuzh4hd+2E8NPPX02mZtn1VuREQToYe/ZdlJy+J3uCpfaiGF05e7B8W0iXbQHmg==",
        "dependencies": {}
      },
      "set-blocking@2.0.0": {
        "integrity": "sha512-KiKBS8AnWGEyLzofFfmvKwpdPzqiy16LvQfK3yv/fVH7Bj13/wl3JSR1J+rfgRE9q7xUJK4qvgS8raSOeLUehw==",
        "dependencies": {}
      },
      "string-width@2.1.1": {
        "integrity": "sha512-nOqH59deCq9SRHlxq1Aw85Jnt4w6KvLKqWVik6oA9ZklXLNIOlqg4F2yrT1MVaTjAqvVwdfeZ7w7aCvJD7ugkw==",
        "dependencies": {
          "is-fullwidth-code-point": "is-fullwidth-code-point@2.0.0",
          "strip-ansi": "strip-ansi@4.0.0"
        }
      },
      "string-width@4.2.3": {
        "integrity": "sha512-wKyQRQpjJ0sIp62ErSZdGsjMJWsap5oRNihHhu6G7JVO/9jIB6UyevL+tXuOqrng8j/cxKTWyWUwvSTriiZz/g==",
        "dependencies": {
          "emoji-regex": "emoji-regex@8.0.0",
          "is-fullwidth-code-point": "is-fullwidth-code-point@3.0.0",
          "strip-ansi": "strip-ansi@6.0.1"
        }
      },
      "strip-ansi@4.0.0": {
        "integrity": "sha512-4XaJ2zQdCzROZDivEVIDPkcQn8LMFSa8kj8Gxb/Lnwzv9A8VctNZ+lfivC/sV3ivW8ElJTERXZoPBRrZKkNKow==",
        "dependencies": { "ansi-regex": "ansi-regex@3.0.1" }
      },
      "strip-ansi@6.0.1": {
        "integrity": "sha512-Y38VPSHcqkFrCpFnQ9vuSXmquuv5oXOKpGeT6aGrr3o3Gc9AlVa6JBfUSOCnbxGGZF+/0ooI7KrPuUSztUdU5A==",
        "dependencies": { "ansi-regex": "ansi-regex@5.0.1" }
      },
      "strip-final-newline@2.0.0": {
        "integrity": "sha512-BrpvfNAE3dcvq7ll3xVumzjKjZQ5tI1sEUIKr3Uoks0XUl45St3FlatVqef9prk4jRDzhW6WZg+3bk93y6pLjA==",
        "dependencies": {}
      },
      "which-module@2.0.0": {
        "integrity": "sha512-B+enWhmw6cjfVC7kS8Pj9pCrKSc5txArRyaYGe088shv/FGWH+0Rjx/xPgtsWfsUtS27FkP697E4DDhgrgoc0Q==",
        "dependencies": {}
      },
      "wrap-ansi@6.2.0": {
        "integrity": "sha512-r6lPcBGxZXlIcymEu7InxDMhdW0KDxpLgoFLcguasxCaJ/SOIZwINatK9KY/tf+ZrlywOKU0UDj3ATXUBfxJXA==",
        "dependencies": {
          "ansi-styles": "ansi-styles@4.3.0",
          "string-width": "string-width@4.2.3",
          "strip-ansi": "strip-ansi@6.0.1"
        }
      },
      "y18n@4.0.3": {
        "integrity": "sha512-JKhqTOwSrqNA1NY5lSztJ1GrBiUodLMmIZuLiDaMRJ+itFd+ABVE8XBjOvIWL+rSqNDC74LCSFmlb/U4UZ4hJQ==",
        "dependencies": {}
      },
      "yargs-parser@18.1.3": {
        "integrity": "sha512-o50j0JeToy/4K6OZcaQmW6lyXXKhq7csREXcDwk2omFPJEwUNOVtJKvmDr9EI1fAJZUyZcRF7kxGBWmRXudrCQ==",
        "dependencies": {
          "camelcase": "camelcase@5.3.1",
          "decamelize": "decamelize@1.2.0"
        }
      },
      "yargs@15.4.1": {
        "integrity": "sha512-aePbxDmcYW++PaqBsJ+HYUFwCdv4LVvdnhBy78E57PIor8/OVvhMrADFFEDh8DHDFRv/O9i3lPhsENjO7QX0+A==",
        "dependencies": {
          "cliui": "cliui@6.0.0",
          "decamelize": "decamelize@1.2.0",
          "find-up": "find-up@4.1.0",
          "get-caller-file": "get-caller-file@2.0.5",
          "require-directory": "require-directory@2.1.1",
          "require-main-filename": "require-main-filename@2.0.0",
          "set-blocking": "set-blocking@2.0.0",
          "string-width": "string-width@4.2.3",
          "which-module": "which-module@2.0.0",
          "y18n": "y18n@4.0.3",
          "yargs-parser": "yargs-parser@18.1.3"
        }
      }
    }
  }
}
"#;
    temp_dir.write("deno.lock", lock_file_content);

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("cache")
      .arg("--lock-write")
      .arg("--quiet")
      .arg("npm:cowsay@1.5.0")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.is_empty());
    assert_eq!(
      lock_file_content,
      std::fs::read_to_string(temp_dir.path().join("deno.lock")).unwrap()
    );
  }

  #[test]
  fn auto_discover_lock_file() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();
    let temp_dir = util::TempDir::new();

    // write empty config file
    temp_dir.write("deno.json", "{}");

    // write a lock file with borked integrity
    let lock_file_content = r#"{
    "version": "2",
    "remote": {},
    "npm": {
      "specifiers": { "@denotest/bin": "@denotest/bin@1.0.0" },
      "packages": {
        "@denotest/bin@1.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": {}
        }
      }
    }
  }"#;
    temp_dir.write("deno.lock", lock_file_content);

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--unstable")
      .arg("-A")
      .arg("npm:@denotest/bin/cli-esm")
      .arg("test")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(10));

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(
      "Integrity check failed for npm package: \"@denotest/bin@1.0.0\""
    ));
  }

  #[test]
  fn peer_deps_with_copied_folders_and_lockfile() {
    let _server = http_server();

    let deno_dir = util::new_deno_dir();
    let temp_dir = util::TempDir::new();

    // write empty config file
    temp_dir.write("deno.json", "{}");
    let test_folder_path = test_util::testdata_path()
      .join("npm")
      .join("peer_deps_with_copied_folders");
    let main_contents =
      std::fs::read_to_string(test_folder_path.join("main.ts")).unwrap();
    temp_dir.write("./main.ts", main_contents);

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());

    let expected_output =
      std::fs::read_to_string(test_folder_path.join("main.out")).unwrap();

    assert_eq!(String::from_utf8(output.stderr).unwrap(), expected_output);

    assert!(temp_dir.path().join("deno.lock").exists());
    let grandchild_path = deno_dir
      .path()
      .join("npm")
      .join("localhost_4545")
      .join("npm")
      .join("registry")
      .join("@denotest")
      .join("peer-dep-test-grandchild");
    assert!(grandchild_path.join("1.0.0").exists());
    assert!(grandchild_path.join("1.0.0_1").exists()); // copy folder, which is hardlinked

    // run again
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "1\n2\n");
    assert!(output.status.success());

    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--reload")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), expected_output);
    assert!(output.status.success());

    // now run with local node modules
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--node-modules-dir")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "1\n2\n");
    assert!(output.status.success());

    let deno_folder = temp_dir.path().join("node_modules").join(".deno");
    assert!(deno_folder
      .join("@denotest+peer-dep-test-grandchild@1.0.0")
      .exists());
    assert!(deno_folder
      .join("@denotest+peer-dep-test-grandchild@1.0.0_1")
      .exists()); // copy folder

    // now again run with local node modules
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--node-modules-dir")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "1\n2\n");

    // now ensure it works with reloading
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--node-modules-dir")
      .arg("--reload")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), expected_output);

    // now ensure it works with reloading and no lockfile
    let deno = util::deno_cmd_with_deno_dir(&deno_dir)
      .current_dir(temp_dir.path())
      .arg("run")
      .arg("--node-modules-dir")
      .arg("--no-lock")
      .arg("--reload")
      .arg("-A")
      .arg("main.ts")
      .envs(env_vars_for_npm_tests())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let output = deno.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), expected_output,);
    assert!(output.status.success());
  }

  itest!(info_peer_deps {
    args: "info --quiet npm/peer_deps_with_copied_folders/main.ts",
    output: "npm/peer_deps_with_copied_folders/main_info.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(info_peer_deps_json {
    args: "info --quiet --json npm/peer_deps_with_copied_folders/main.ts",
    output: "npm/peer_deps_with_copied_folders/main_info_json.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });

  itest!(create_require {
    args: "run --reload --allow-read npm/create_require/main.ts",
    output: "npm/create_require/main.out",
    exit_code: 0,
    envs: env_vars_for_npm_tests(),
    http_server: true,
  });
}
