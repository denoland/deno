// Copyright 2018-2025 the Deno authors. MIT license.

use test_util as util;
use test_util::assert_contains;
use test_util::assert_not_contains;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn install_basic() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();

  // ensure a lockfile doesn't get created or updated locally
  temp_dir.write("deno.json", "{}");

  let output = context
    .new_command()
    .args("install --check --name echo_test -g http://localhost:4545/echo.ts")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  assert_contains!(output_text, "✅ Successfully installed echo_test");

  // no lockfile should be created locally
  assert!(!temp_dir.path().join("deno.lock").exists());

  let mut file_path = temp_dir.path().join(".deno/bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = file_path.read_to_string();
  // ensure there's a trailing newline so the shell script can be
  // more versatile.
  assert_eq!(content.chars().last().unwrap(), '\n');

  if cfg!(windows) {
    assert_contains!(
      content,
      r#""run" "--check" "--no-config" "http://localhost:4545/echo.ts""#
    );
  } else {
    assert_contains!(
      content,
      r#"run --check --no-config 'http://localhost:4545/echo.ts'"#
    );
  }

  // now uninstall
  context
    .new_command()
    .args("uninstall -g echo_test")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  // ensure local lockfile still doesn't exist
  assert!(!temp_dir.path().join("deno.lock").exists());
  // ensure uninstall occurred
  assert!(!file_path.exists());
}

#[test]
fn install_basic_global() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();

  // ensure a lockfile doesn't get created or updated locally
  temp_dir.write("deno.json", "{}");

  let output = context
    .new_command()
    .args(
      "install --global --check --name echo_test http://localhost:4545/echo.ts",
    )
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  assert_not_contains!(
    output_text,
    "`deno install` behavior will change in Deno 2. To preserve the current behavior use the `-g` or `--global` flag."
  );

  // no lockfile should be created locally
  assert!(!temp_dir.path().join("deno.lock").exists());

  let mut file_path = temp_dir.path().join(".deno/bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = file_path.read_to_string();
  // ensure there's a trailing newline so the shell script can be
  // more versatile.
  assert_eq!(content.chars().last().unwrap(), '\n');

  if cfg!(windows) {
    assert_contains!(
      content,
      r#""run" "--check" "--no-config" "http://localhost:4545/echo.ts""#
    );
  } else {
    assert_contains!(
      content,
      r#"run --check --no-config 'http://localhost:4545/echo.ts'"#
    );
  }

  // now uninstall
  context
    .new_command()
    .args("uninstall -g echo_test")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  // ensure local lockfile still doesn't exist
  assert!(!temp_dir.path().join("deno.lock").exists());
  // ensure uninstall occurred
  assert!(!file_path.exists());
}

#[test]
fn install_custom_dir_env_var() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();

  context
    .new_command()
    .current_dir(util::root_path()) // different cwd
    .args("install --check --name echo_test -g http://localhost:4545/echo.ts")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", temp_dir_str.as_str()),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let mut file_path = temp_dir.path().join("bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = file_path.read_to_string();
  if cfg!(windows) {
    assert_contains!(
      content,
      r#""run" "--check" "--no-config" "http://localhost:4545/echo.ts""#
    );
  } else {
    assert_contains!(
      content,
      r#"run --check --no-config 'http://localhost:4545/echo.ts'"#
    );
  }
}

#[test]
fn installer_test_custom_dir_with_bin() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();
  let temp_dir_with_bin = temp_dir.path().join("bin").to_string();

  context
    .new_command()
    .current_dir(util::root_path()) // different cwd
    .args("install --check --name echo_test -g http://localhost:4545/echo.ts")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", temp_dir_with_bin.as_str()),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let mut file_path = temp_dir.path().join("bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = file_path.read_to_string();
  if cfg!(windows) {
    assert_contains!(
      content,
      r#""run" "--check" "--no-config" "http://localhost:4545/echo.ts""#
    );
  } else {
    assert_contains!(
      content,
      r#"run --check --no-config 'http://localhost:4545/echo.ts'"#
    );
  }
}

#[test]
fn installer_test_local_module_run() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();
  let echo_ts_str = util::testdata_path().join("echo.ts").to_string();

  context
    .new_command()
    .current_dir(util::root_path())
    .args_vec([
      "install",
      "-g",
      "--name",
      "echo_test",
      "--root",
      temp_dir_str.as_str(),
      echo_ts_str.as_str(),
      "hello",
    ])
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let bin_dir = temp_dir.path().join("bin");
  let mut file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
  let output = context
    .new_command()
    .name(&file_path)
    .current_dir(temp_dir.path())
    .args("foo")
    .env("PATH", util::target_dir())
    .run();
  output.assert_matches_text("hello, foo");
  output.assert_exit_code(0);
}

#[test]
fn installer_test_remote_module_run() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let root_dir = temp_dir.path().join("root");
  let bin_dir = root_dir.join("bin");
  context
    .new_command()
    .args("install --name echo_test --root ./root -g http://localhost:4545/echo.ts hello")
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  let mut bin_file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    bin_file_path = bin_file_path.with_extension("cmd");
  }
  assert!(bin_file_path.exists());
  let output = context
    .new_command()
    .name(&bin_file_path)
    .current_dir(root_dir)
    .args("foo")
    .env("PATH", util::target_dir())
    .run();
  output.assert_matches_text("hello, foo");
  output.assert_exit_code(0);

  // now uninstall with the relative path
  context
    .new_command()
    .args("uninstall -g --root ./root echo_test")
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  assert!(!bin_file_path.exists());
}

#[test]
fn check_local_by_default() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();
  let script_path =
    util::testdata_path().join("./install/check_local_by_default.ts");
  let script_path_str = script_path.to_string_lossy().to_string();
  context
    .new_command()
    .args_vec(["install", "-g", "--allow-import", script_path_str.as_str()])
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
}

#[test]
fn check_local_by_default2() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();
  let script_path =
    util::testdata_path().join("./install/check_local_by_default2.ts");
  let script_path_str = script_path.to_string_lossy().to_string();
  context
    .new_command()
    .args_vec(["install", "-g", "--allow-import", script_path_str.as_str()])
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("NO_COLOR", "1"),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
}

#[test]
fn show_prefix_hint_on_global_install() {
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .add_jsr_env_vars()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let temp_dir_str = temp_dir.path().to_string();

  let env_vars = [
    ("HOME", temp_dir_str.as_str()),
    ("USERPROFILE", temp_dir_str.as_str()),
    ("DENO_INSTALL_ROOT", ""),
  ];

  for pkg_req in ["npm:@denotest/bin", "jsr:@denotest/add"] {
    let name = pkg_req.split_once('/').unwrap().1;
    let pkg = pkg_req.split_once(':').unwrap().1;

    // try with prefix and ensure that the installation succeeds
    context
      .new_command()
      .args_vec(["install", "-g", "--name", name, pkg_req])
      .envs(env_vars)
      .run()
      .skip_output_check()
      .assert_exit_code(0);

    // try without the prefix and ensure that the installation fails with the appropriate error
    // message
    let output = context
      .new_command()
      .args_vec(["install", "-g", "--name", name, pkg])
      .envs(env_vars)
      .run();
    output.assert_exit_code(1);

    let output_text = output.combined_output();
    let expected_text =
      format!("error: {pkg} is missing a prefix. Did you mean `deno install -g {pkg_req}`?");
    assert_contains!(output_text, &expected_text);
  }

  // try a pckage not in npm and jsr to make sure the appropriate error message still appears
  let output = context
    .new_command()
    .args_vec(["install", "-g", "package-that-does-not-exist"])
    .envs(env_vars)
    .run();
  output.assert_exit_code(1);

  let output_text = output.combined_output();
  assert_contains!(output_text, "error: Module not found");
}
