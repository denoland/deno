// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs::File;
use std::process::Command;
use test_util as util;
use test_util::TempDir;
use util::assert_contains;
use util::env_vars_for_npm_tests;
use util::TestContextBuilder;

#[test]
fn compile() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  // try this twice to ensure it works with the cache
  for _ in 0..2 {
    let output = util::deno_cmd_with_deno_dir(&dir)
      .current_dir(util::root_path())
      .arg("compile")
      .arg("--unstable")
      .arg("--output")
      .arg(&exe)
      .arg("./test_util/std/examples/welcome.ts")
      .stdout(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    assert!(output.status.success());
    let output = Command::new(&exe)
      .stdout(std::process::Stdio::piped())
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, "Welcome to Deno!\n".as_bytes());
  }
}

#[test]
fn standalone_args() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/args.ts")
    .arg("a")
    .arg("b")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .arg("foo")
    .arg("--bar")
    .arg("--unstable")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"a\nb\nfoo\n--bar\n--unstable\n");
}

#[test]
fn standalone_error() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("error.exe")
  } else {
    dir.path().join("error")
  };
  let testdata_path = util::testdata_path();
  let output = util::deno_cmd()
    .current_dir(&testdata_path)
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_error.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(output.stdout, b"");
  let stderr = String::from_utf8(output.stderr).unwrap();
  let stderr = util::strip_ansi_codes(&stderr).to_string();
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert_contains!(stderr, "error: Uncaught Error: boom!");
  assert_contains!(stderr, "throw new Error(\"boom!\");");
  assert_contains!(stderr, "\n    at boom (file://");
  assert_contains!(stderr, "standalone_error.ts:2:11");
  assert_contains!(stderr, "at foo (file://");
  assert_contains!(stderr, "standalone_error.ts:5:5");
  assert_contains!(stderr, "standalone_error.ts:7:1");
}

#[test]
fn standalone_error_module_with_imports() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("error.exe")
  } else {
    dir.path().join("error")
  };
  let testdata_path = util::testdata_path();
  let output = util::deno_cmd()
    .current_dir(&testdata_path)
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_error_module_with_imports_1.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(output.stdout, b"hello\n");
  let stderr = String::from_utf8(output.stderr).unwrap();
  let stderr = util::strip_ansi_codes(&stderr).to_string();
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert_contains!(stderr, "error: Uncaught Error: boom!");
  assert_contains!(stderr, "throw new Error(\"boom!\");");
  assert_contains!(stderr, "\n    at file://");
  assert_contains!(stderr, "standalone_error_module_with_imports_2.ts:2:7");
}

#[test]
fn standalone_load_datauri() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("load_datauri.exe")
  } else {
    dir.path().join("load_datauri")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_import_datauri.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Hello Deno!\n");
}

// https://github.com/denoland/deno/issues/13704
#[test]
fn standalone_follow_redirects() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("follow_redirects.exe")
  } else {
    dir.path().join("follow_redirects")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_follow_redirects.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Hello\n");
}

#[test]
fn compile_with_file_exists_error() {
  let dir = TempDir::new();
  let output_path = if cfg!(windows) {
    dir.path().join(r"args\")
  } else {
    dir.path().join("args/")
  };
  let file_path = dir.path().join("args");
  File::create(&file_path).unwrap();
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&output_path)
    .arg("./compile/args.ts")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let expected_stderr = format!(
    concat!(
      "Could not compile to file '{}' because its parent directory ",
      "is an existing file. You can use the `--output <file-path>` flag to ",
      "provide an alternative name.\n",
    ),
    file_path.display(),
  );
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert_contains!(stderr, &expected_stderr);
}

#[test]
fn compile_with_directory_exists_error() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::create_dir(&exe).unwrap();
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/args.ts")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let expected_stderr = format!(
    concat!(
      "Could not compile to file '{}' because a directory exists with ",
      "the same name. You can use the `--output <file-path>` flag to ",
      "provide an alternative name."
    ),
    exe.display()
  );
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert_contains!(stderr, &expected_stderr);
}

#[test]
fn compile_with_conflict_file_exists_error() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::write(&exe, b"SHOULD NOT BE OVERWRITTEN").unwrap();
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/args.ts")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let expected_stderr = format!(
    concat!(
      "Could not compile to file '{}' because the file already exists ",
      "and cannot be overwritten. Please delete the existing file or ",
      "use the `--output <file-path` flag to provide an alternative name."
    ),
    exe.display()
  );
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert_contains!(stderr, &expected_stderr);
  assert!(std::fs::read(&exe)
    .unwrap()
    .eq(b"SHOULD NOT BE OVERWRITTEN"));
}

#[test]
fn compile_and_overwrite_file() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/args.ts")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(&exe.exists());

  let recompile_output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/args.ts")
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(recompile_output.status.success());
}

#[test]
fn standalone_runtime_flags() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("flags.exe")
  } else {
    dir.path().join("flags")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--seed")
    .arg("1")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_runtime_flags.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stdout_str = String::from_utf8(output.stdout).unwrap();
  assert_eq!(util::strip_ansi_codes(&stdout_str), "0.147205063401058\n");
  let stderr_str = String::from_utf8(output.stderr).unwrap();
  assert_contains!(
    util::strip_ansi_codes(&stderr_str),
    "PermissionDenied: Requires write access"
  );
}

#[test]
fn standalone_ext_flag_ts() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("ext_flag_ts.exe")
  } else {
    dir.path().join("ext_flag_ts")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--ext")
    .arg("ts")
    .arg("--output")
    .arg(&exe)
    .arg("./file_extensions/ts_without_extension")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = String::from_utf8(output.stdout).unwrap();
  assert_eq!(
    util::strip_ansi_codes(&stdout_str),
    "executing typescript with no extension\n"
  );
}

#[test]
fn standalone_ext_flag_js() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("ext_flag_js.exe")
  } else {
    dir.path().join("ext_flag_js")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--ext")
    .arg("js")
    .arg("--output")
    .arg(&exe)
    .arg("./file_extensions/js_without_extension")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = String::from_utf8(output.stdout).unwrap();
  assert_eq!(
    util::strip_ansi_codes(&stdout_str),
    "executing javascript with no extension\n"
  );
}

#[test]
fn standalone_import_map() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("import_map.exe")
  } else {
    dir.path().join("import_map")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--import-map")
    .arg("compile/standalone_import_map.json")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_import_map.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn standalone_import_map_config_file() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("import_map.exe")
  } else {
    dir.path().join("import_map")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--allow-read")
    .arg("--config")
    .arg("compile/standalone_import_map_config.json")
    .arg("--output")
    .arg(&exe)
    .arg("./compile/standalone_import_map.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
// https://github.com/denoland/deno/issues/12670
fn skip_rebundle() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("hello_world.exe")
  } else {
    dir.path().join("hello_world")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./run/001_hello.js")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());

  //no "Bundle testdata_path/run/001_hello.js" in output
  assert!(!String::from_utf8(output.stderr).unwrap().contains("Bundle"));

  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, "Hello World\n".as_bytes());
}

#[test]
fn check_local_by_default() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg(util::testdata_path().join("./compile/check_local_by_default.ts"))
    .status()
    .unwrap();
  assert!(status.success());
}

#[test]
fn check_local_by_default2() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .env("NO_COLOR", "1")
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg(util::testdata_path().join("./compile/check_local_by_default2.ts"))
    .output()
    .unwrap();
  assert!(!output.status.success());
  let stdout = String::from_utf8(output.stdout).unwrap();
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(stdout.is_empty());
  assert_contains!(
    stderr,
    r#"error: TS2322 [ERROR]: Type '12' is not assignable to type '"b"'."#
  );
}

#[test]
fn workers_basic() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("basic.exe")
  } else {
    dir.path().join("basic")
  };
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--no-check")
    .arg("--output")
    .arg(&exe)
    .arg(util::testdata_path().join("./compile/workers/basic.ts"))
    .output()
    .unwrap();
  assert!(output.status.success());

  let output = Command::new(&exe).output().unwrap();
  assert!(output.status.success());
  let expected = std::fs::read_to_string(
    util::testdata_path().join("./compile/workers/basic.out"),
  )
  .unwrap();
  assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn workers_not_in_module_map() {
  let context = TestContextBuilder::for_npm()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();
  let exe = if cfg!(windows) {
    temp_dir.path().join("not_in_module_map.exe")
  } else {
    temp_dir.path().join("not_in_module_map")
  };
  let main_path =
    util::testdata_path().join("./compile/workers/not_in_module_map.ts");
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      &main_path.to_string_lossy(),
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .command_name(exe.to_string_lossy())
    .env("NO_COLOR", "")
    .run();
  output.assert_exit_code(1);
  output.assert_matches_text(concat!(
    "error: Uncaught (in worker \"\") Module not found: [WILDCARD]",
    "error: Uncaught (in promise) Error: Unhandled error in child worker.\n[WILDCARD]"
  ));
}

#[test]
fn workers_with_include_flag() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("workers_with_include_flag.exe")
  } else {
    dir.path().join("workers_with_include_flag")
  };
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--include")
    .arg(util::testdata_path().join("./compile/workers/worker.ts"))
    .arg("--output")
    .arg(&exe)
    .arg(util::testdata_path().join("./compile/workers/not_in_module_map.ts"))
    .output()
    .unwrap();
  assert!(output.status.success());

  let output = Command::new(&exe).env("NO_COLOR", "").output().unwrap();
  assert!(output.status.success());
  let expected_stdout =
    concat!("Hello from worker!\n", "Received 42\n", "Closing\n");
  assert_eq!(&String::from_utf8(output.stdout).unwrap(), expected_stdout);
}

#[test]
fn dynamic_import() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("dynamic_import.exe")
  } else {
    dir.path().join("dynamic_import")
  };
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--output")
    .arg(&exe)
    .arg(util::testdata_path().join("./compile/dynamic_imports/main.ts"))
    .output()
    .unwrap();
  assert!(output.status.success());

  let output = Command::new(&exe).env("NO_COLOR", "").output().unwrap();
  assert!(output.status.success());
  let expected = std::fs::read_to_string(
    util::testdata_path().join("./compile/dynamic_imports/main.out"),
  )
  .unwrap();
  assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn dynamic_import_unanalyzable() {
  let _guard = util::http_server();
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("dynamic_import_unanalyzable.exe")
  } else {
    dir.path().join("dynamic_import_unanalyzable")
  };
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--allow-read")
    .arg("--include")
    .arg(util::testdata_path().join("./compile/dynamic_imports/import1.ts"))
    .arg("--output")
    .arg(&exe)
    .arg(
      util::testdata_path()
        .join("./compile/dynamic_imports/main_unanalyzable.ts"),
    )
    .output()
    .unwrap();
  assert!(output.status.success());

  let output = Command::new(&exe).env("NO_COLOR", "").output().unwrap();
  assert!(output.status.success());
  let expected = std::fs::read_to_string(
    util::testdata_path().join("./compile/dynamic_imports/main.out"),
  )
  .unwrap();
  assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

itest!(npm_specifiers_errors_no_unstable {
  args: "compile -A --quiet npm/cached_only/main.ts",
  output_str: Some(
    concat!(
      "error: Using npm specifiers with deno compile requires the --unstable flag.",
      "\n\n",
      "Caused by:\n",
      "    npm specifiers have not yet been implemented for this sub command (https://github.com/denoland/deno/issues/15960). Found: npm:chalk@5.0.1\n"
    )
  ),
  exit_code: 1,
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

#[test]
fn compile_npm_specifiers() {
  let context = TestContextBuilder::for_npm()
    .use_sync_npm_download()
    .use_temp_cwd()
    .build();

  let temp_dir = context.temp_dir();
  temp_dir.write(
    "main.ts",
    concat!(
      "import path from 'node:path';\n",
      "import { getValue, setValue } from 'npm:@denotest/esm-basic';\n",
      "import getValueDefault from 'npm:@denotest/esm-import-cjs-default';\n",
      "setValue(2);\n",
      "console.log(path.join('testing', 'this'));",
      "console.log(getValue());",
      "console.log(getValueDefault());",
    ),
  );

  let binary_path = if cfg!(windows) {
    temp_dir.path().join("binary.exe")
  } else {
    temp_dir.path().join("binary")
  };

  // try with and without --node-modules-dir
  let compile_commands = &[
    "compile --unstable --output binary main.ts",
    "compile --unstable --node-modules-dir --output binary main.ts",
  ];

  for compile_command in compile_commands {
    let output = context.new_command().args(compile_command).run();
    output.assert_exit_code(0);
    output.skip_output_check();

    let output = context
      .new_command()
      .command_name(binary_path.to_string_lossy())
      .run();
    output.assert_matches_text(
      r#"Node esm importing node cjs
===========================
{
  default: [Function (anonymous)],
  named: [Function (anonymous)],
  MyClass: [class MyClass]
}
{ default: [Function (anonymous)], named: [Function (anonymous)] }
[Module: null prototype] {
  MyClass: [class MyClass],
  __esModule: true,
  default: {
    default: [Function (anonymous)],
    named: [Function (anonymous)],
    MyClass: [class MyClass]
  },
  named: [Function (anonymous)]
}
[Module: null prototype] {
  __esModule: true,
  default: { default: [Function (anonymous)], named: [Function (anonymous)] },
  named: [Function (anonymous)]
}
===========================
static method
testing[WILDCARD]this
2
5
"#,
    );
  }

  // try with a package.json
  temp_dir.remove_dir_all("node_modules");
  temp_dir.write(
    "main.ts",
    concat!(
      "import { getValue, setValue } from '@denotest/esm-basic';\n",
      "setValue(2);\n",
      "console.log(getValue());",
    ),
  );
  temp_dir.write(
    "package.json",
    r#"{ "dependencies": { "@denotest/esm-basic": "1" } }"#,
  );

  let output = context
    .new_command()
    .args("compile --unstable --output binary main.ts")
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context
    .new_command()
    .command_name(binary_path.to_string_lossy())
    .run();
  output.assert_matches_text("2\n");
}

#[test]
fn compile_npm_file_system() {
  let context = TestContextBuilder::for_npm()
    .use_sync_npm_download()
    .use_temp_cwd()
    .build();

  let temp_dir = context.temp_dir();
  let testdata_path = context.testdata_path();
  let main_path = testdata_path.join("compile/npm_fs/main.ts");

  // compile
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "-A",
      "--node-modules-dir",
      "--unstable",
      "--output",
      "binary",
      &main_path.to_string_lossy(),
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

  // run
  let binary_path = if cfg!(windows) {
    temp_dir.path().join("binary.exe")
  } else {
    temp_dir.path().join("binary")
  };
  let output = context
    .new_command()
    .command_name(binary_path.to_string_lossy())
    .run();
  output.assert_matches_file("compile/npm_fs/main.out");
}
