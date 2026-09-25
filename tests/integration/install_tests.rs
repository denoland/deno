// Copyright 2018-2026 the Deno authors. MIT license.

use test_util as util;
use util::TestContext;
use util::TestContextBuilder;
use util::assert_contains;
use util::assert_not_contains;
use util::test;

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

  // shim should point to a per-command config in .<name>/deno.json
  let config_dir = temp_dir.path().join(".deno").join("bin").join(".echo_test");
  let config_path = config_dir.join("deno.json");
  assert!(config_path.exists());
  if cfg!(windows) {
    assert_contains!(
      content,
      &format!(
        r#""run" "--check" "--config" "{config_path}" "http://localhost:4545/echo.ts""#
      )
    );
  } else {
    assert_contains!(
      content,
      &format!(
        "run --check --config {config_path} 'http://localhost:4545/echo.ts'"
      )
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
  // ensure config dir was cleaned up
  assert!(!config_dir.exists());
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

  // shim should point to a per-command config in .<name>/deno.json
  let config_path = temp_dir
    .path()
    .join(".deno")
    .join("bin")
    .join(".echo_test")
    .join("deno.json");
  if cfg!(windows) {
    assert_contains!(
      content,
      &format!(
        r#""run" "--check" "--config" "{config_path}" "http://localhost:4545/echo.ts""#
      )
    );
  } else {
    assert_contains!(
      content,
      &format!(
        "run --check --config {config_path} 'http://localhost:4545/echo.ts'"
      )
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
fn install_global_npm_lifecycle_scripts_prompt() {
  if !util::pty::Pty::is_supported() {
    return;
  }

  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  context
    .new_command()
    .args(
      "install --global --root ./bins --name lifecycle-scripts-simple npm:@denotest/lifecycle-scripts-simple",
    )
    .with_pty(|mut console| {
      console.expect(
        "Run lifecycle scripts for npm:@denotest/lifecycle-scripts-simple@1.0.0?",
      );
      console.write_raw("yl");
      console.expect("Successfully installed lifecycle-scripts-simple");
    });

  let output = context
    .new_command()
    .name(if cfg!(windows) {
      "./bins/bin/lifecycle-scripts-simple.cmd"
    } else {
      "./bins/bin/lifecycle-scripts-simple"
    })
    .run();
  output.assert_exit_code(0);
  output.assert_matches_text("postinstall works\n");

  let config_text = temp_dir
    .path()
    .join("bins")
    .join("bin")
    .join(".lifecycle-scripts-simple")
    .join("deno.json")
    .read_to_string();
  assert_contains!(
    config_text,
    r#""allowScripts": ["npm:@denotest/lifecycle-scripts-simple@1.0.0"]"#
  );
}

// Regression test for #32798: a relative `--import-map` passed to a global
// install (typically via `deno task`) must resolve against the user's cwd,
// not the generated `~/.deno/bin/.<name>/` install dir.
#[test]
fn install_global_from_task_with_relative_import_map() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "deno.json",
    r#"{
  "tasks": {
    "install": "deno install -A --global --root ./root --import-map imports.json -n test main.ts"
  }
}
"#,
  );
  temp_dir.write(
    "imports.json",
    r#"{
  "imports": {
    "@fixture": "./fixture.ts"
  }
}
"#,
  );
  temp_dir.write("fixture.ts", "export const value = 1;\n");
  temp_dir.write(
    "main.ts",
    "import { value } from \"@fixture\";\nconsole.log(value);\n",
  );

  let output = context.new_command().args("task install").run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  assert_contains!(output_text, "✅ Successfully installed test");
  assert_not_contains!(output_text, ".test/imports.json");

  let mut file_path = temp_dir.path().join("root/bin/test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
}

// Companion to the regression test above. An absolute `--import-map` is *not*
// affected by the #32798 cwd swap (there is no relative path to re-anchor), so
// this test passes with or without the fix. Its purpose is to guard against a
// future refactor breaking the `resolve_url_or_path` round-trip for absolute
// paths: `main.ts` resolves `@fixture` through the import map during the
// install-time dependency cache, so a mangled path would fail the install here.
#[test]
fn install_global_with_absolute_import_map() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();

  temp_dir.write(
    "imports.json",
    r#"{
  "imports": {
    "@fixture": "./fixture.ts"
  }
}
"#,
  );
  temp_dir.write("fixture.ts", "export const value = 1;\n");
  temp_dir.write(
    "main.ts",
    "import { value } from \"@fixture\";\nconsole.log(value);\n",
  );

  let import_map_path = temp_dir.path().join("imports.json");
  let output = context
    .new_command()
    .args_vec([
      "install",
      "-A",
      "--global",
      "--root",
      "./root",
      "--import-map",
      import_map_path.to_string().as_str(),
      "-n",
      "test",
      "main.ts",
    ])
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  assert_contains!(output_text, "✅ Successfully installed test");

  let mut file_path = temp_dir.path().join("root/bin/test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
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
  // shim should point to a per-command config
  let config_path = temp_dir
    .path()
    .join("bin")
    .canonicalize()
    .join(".echo_test")
    .join("deno.json");
  if cfg!(windows) {
    assert_contains!(
      content,
      &format!(
        r#""run" "--check" "--config" "{config_path}" "http://localhost:4545/echo.ts""#
      )
    );
  } else {
    assert_contains!(
      content,
      &format!(
        "run --check --config {config_path} 'http://localhost:4545/echo.ts'"
      )
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
  // shim should point to a per-command config
  let config_path = temp_dir
    .path()
    .join("bin")
    .canonicalize()
    .join(".echo_test")
    .join("deno.json");
  if cfg!(windows) {
    assert_contains!(
      content,
      &format!(
        r#""run" "--check" "--config" "{config_path}" "http://localhost:4545/echo.ts""#
      )
    );
  } else {
    assert_contains!(
      content,
      &format!(
        "run --check --config {config_path} 'http://localhost:4545/echo.ts'"
      )
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
      "--",
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
    .args("install --name echo_test --root ./root -g http://localhost:4545/echo.ts -- hello")
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
  let script_path_str = script_path.to_string_lossy().into_owned();
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
  let script_path_str = script_path.to_string_lossy().into_owned();
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
fn unprefixed_global_install_defaults_to_npm() {
  // Unprefixed package names in `deno install -g` default to the npm
  // registry, matching `deno add` and local `deno install`.
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

  // Bare npm name (no prefix) installs successfully from npm.
  context
    .new_command()
    .args_vec(["install", "-g", "--name", "bin", "@denotest/bin"])
    .envs(env_vars)
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  // A package that doesn't exist on npm produces the npm "does not exist"
  // error rather than the legacy "missing a prefix" hint.
  let output = context
    .new_command()
    .args_vec(["install", "-g", "package-that-does-not-exist"])
    .envs(env_vars)
    .run();
  output.assert_exit_code(1);
  assert_contains!(
    output.combined_output(),
    "npm package 'package-that-does-not-exist' does not exist"
  );
}

#[test]
fn installer_multiple() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let root_dir = temp_dir.path().join("root");
  let bin_dir = root_dir.join("bin");
  context
    .new_command()
    .args("install --root ./root -g http://localhost:4545/echo.ts http://localhost:4545/cat.ts")
    .run()
    .assert_matches_text("[WILDCARD]Successfully installed echo[WILDCARD]Successfully installed cat[WILDCARD]")
    .assert_exit_code(0);
  for name in ["echo", "cat"] {
    let mut bin_file_path = bin_dir.join(name);
    if cfg!(windows) {
      bin_file_path = bin_file_path.with_extension("cmd");
    }
    assert!(bin_file_path.exists());
  }
}

#[test]
fn installer_second_module_looks_like_script_argument() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  // in Deno < 3.0, we didn't require `--` before script arguments, so we try
  // to provide a helpful error message for people migrating. Validation
  // happens up-front for the whole entry list, so the migration error fires
  // before any entry is installed.
  context
    .new_command()
    .args("install --root ./root -g http://localhost:4545/echo.ts non_existent")
    .run()
    .assert_matches_text(concat!(
      "error: non_existent is missing a prefix. ",
      "Deno 3.0 requires `--` before script arguments in `deno install -g`. ",
      "Did you mean `deno install -g http://localhost:4545/echo.ts -- non_existent`? ",
      "Or maybe provide a `jsr:` or `npm:` prefix?\n"
    ))
    .assert_exit_code(1);
}

#[test]
fn install_npm_global_config_dir() {
  // installing an npm: package should create a .<name>/ config dir
  // with deno.json containing nodeModulesDir
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let root_dir = temp_dir.path().join("root");
  let bin_dir = root_dir.join("bin");

  context
    .new_command()
    .args("install -g --root ./root --name cli-esm npm:@denotest/bin")
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  // config dir should exist
  let config_dir = bin_dir.join(".cli-esm");
  assert!(config_dir.exists(), "config dir should exist");

  // deno.json should exist with nodeModulesDir
  let deno_json = config_dir.join("deno.json");
  assert!(deno_json.exists(), "deno.json should exist");
  let deno_json_content = deno_json.read_to_string();
  assert_contains!(deno_json_content, "nodeModulesDir");
  assert_contains!(deno_json_content, "manual");
  // should have workspace field to stop workspace discovery
  assert_contains!(deno_json_content, "workspace");

  // shim should use --config pointing to the config dir
  let mut shim_path = bin_dir.join("cli-esm");
  if cfg!(windows) {
    shim_path = shim_path.with_extension("cmd");
  }
  assert!(shim_path.exists(), "shim should exist");
  let shim_content = shim_path.read_to_string();
  assert_contains!(shim_content, "--config");
  assert_contains!(shim_content, ".cli-esm");

  // now uninstall and verify cleanup
  context
    .new_command()
    .args("uninstall -g --root ./root cli-esm")
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  assert!(!shim_path.exists(), "shim should be removed");
  assert!(!config_dir.exists(), "config dir should be removed");
}

#[test]
fn install_npm_global_allow_scripts() {
  // installing with --allow-scripts should run lifecycle scripts
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let root_dir = temp_dir.path().join("root");
  let bin_dir = root_dir.join("bin");

  let output = context
    .new_command()
    .args("install -g --allow-scripts --root ./root npm:@denotest/lifecycle-scripts-simple")
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  assert_contains!(output_text, "Successfully installed");

  // config dir should exist with node_modules
  let config_dir = bin_dir.join(".lifecycle-scripts-simple");
  assert!(config_dir.exists(), "config dir should exist");

  // deno.json should have nodeModulesDir
  let deno_json = config_dir.join("deno.json");
  assert!(deno_json.exists(), "deno.json should exist");
  let deno_json_content = deno_json.read_to_string();
  assert_contains!(deno_json_content, "nodeModulesDir");

  // node_modules should exist (populated by cache_top_level_deps)
  let node_modules = config_dir.join("node_modules");
  assert!(node_modules.exists(), "node_modules should exist");

  // package.json should exist with the dependency
  let package_json = config_dir.join("package.json");
  assert!(package_json.exists(), "package.json should exist");
  let package_json_content = package_json.read_to_string();
  assert_contains!(package_json_content, "@denotest/lifecycle-scripts-simple");
}
