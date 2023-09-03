// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fs::File;
use std::process::Command;
use test_util as util;
use test_util::TempDir;
use util::assert_contains;
use util::TestContextBuilder;

#[test]
fn compile_basic() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  // try this twice to ensure it works with the cache
  for _ in 0..2 {
    let output = context
      .new_command()
      .args_vec([
        "compile",
        "--output",
        &exe.to_string_lossy(),
        "../../../test_util/std/examples/welcome.ts",
      ])
      .run();
    output.assert_exit_code(0);
    output.skip_output_check();
    let output = context.new_command().command_name(&exe).run();
    output.assert_matches_text("Welcome to Deno!\n");
  }

  // now ensure this works when the deno_dir is readonly
  let readonly_dir = dir.path().join("readonly");
  readonly_dir.make_dir_readonly();
  let readonly_sub_dir = readonly_dir.join("sub");

  let output = context
    .new_command()
    // it should fail creating this, but still work
    .env("DENO_DIR", readonly_sub_dir)
    .command_name(exe)
    .run();
  output.assert_matches_text("Welcome to Deno!\n");
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
  assert_contains!(stderr, "standalone_error.ts:2:9");
  assert_contains!(stderr, "at foo (file://");
  assert_contains!(stderr, "standalone_error.ts:5:3");
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
    file_path,
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
    exe
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
      "use the `--output <file-path>` flag to provide an alternative name."
    ),
    exe
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
    .command_name(exe)
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
    "compile --output binary main.ts",
    "compile --node-modules-dir --output binary main.ts",
  ];

  for compile_command in compile_commands {
    let output = context.new_command().args(compile_command).run();
    output.assert_exit_code(0);
    output.skip_output_check();

    let output = context.new_command().command_name(&binary_path).run();
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
    .args("compile --output binary main.ts")
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context.new_command().command_name(binary_path).run();
  output.assert_matches_text("2\n");
}

#[test]
fn compile_npm_file_system() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "compile/npm_fs/main.ts",
    compile_args: vec!["-A"],
    run_args: vec![],
    output_file: "compile/npm_fs/main.out",
    node_modules_dir: true,
    input_name: Some("binary"),
    expected_name: "binary",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_bin_esm() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:@denotest/bin/cli-esm",
    compile_args: vec![],
    run_args: vec!["this", "is", "a", "test"],
    output_file: "npm/deno_run_esm.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cli-esm",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_bin_cjs() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:@denotest/bin/cli-cjs",
    compile_args: vec![],
    run_args: vec!["this", "is", "a", "test"],
    output_file: "npm/deno_run_cjs.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cli-cjs",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_cowsay_main() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0",
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_vfs_implicit_read_permissions() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "compile/vfs_implicit_read_permission/main.ts",
    compile_args: vec![],
    run_args: vec![],
    output_file: "compile/vfs_implicit_read_permission/main.out",
    node_modules_dir: false,
    input_name: Some("binary"),
    expected_name: "binary",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_no_permissions() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0",
    compile_args: vec![],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay_no_permissions.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 1,
  });
}

#[test]
fn compile_npm_cowsay_explicit() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0/cowsay",
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_cowthink() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0/cowthink",
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowthink.out",
    node_modules_dir: false,
    input_name: None,
    expected_name: "cowthink",
    exit_code: 0,
  });
}

struct RunNpmBinCompileOptions<'a> {
  input_specifier: &'a str,
  node_modules_dir: bool,
  output_file: &'a str,
  input_name: Option<&'a str>,
  expected_name: &'a str,
  run_args: Vec<&'a str>,
  compile_args: Vec<&'a str>,
  exit_code: i32,
}

fn run_npm_bin_compile_test(opts: RunNpmBinCompileOptions) {
  let context = TestContextBuilder::for_npm()
    .use_sync_npm_download()
    .use_temp_cwd()
    .build();

  let temp_dir = context.temp_dir();
  let testdata_path = context.testdata_path();
  let main_specifier = if opts.input_specifier.starts_with("npm:") {
    opts.input_specifier.to_string()
  } else {
    testdata_path.join(opts.input_specifier).to_string()
  };

  let mut args = vec!["compile".to_string()];

  args.extend(opts.compile_args.iter().map(|s| s.to_string()));

  if opts.node_modules_dir {
    args.push("--node-modules-dir".to_string());
  }

  if let Some(bin_name) = opts.input_name {
    args.push("--output".to_string());
    args.push(bin_name.to_string());
  }

  args.push(main_specifier);

  // compile
  let output = context.new_command().args_vec(args).run();
  output.assert_exit_code(0);
  output.skip_output_check();

  // delete the npm folder in the DENO_DIR to ensure it's not using it
  context.deno_dir().remove_dir_all("./npm");

  // run
  let binary_path = if cfg!(windows) {
    temp_dir.path().join(format!("{}.exe", opts.expected_name))
  } else {
    temp_dir.path().join(opts.expected_name)
  };
  let output = context
    .new_command()
    .command_name(binary_path)
    .args_vec(opts.run_args)
    .run();
  output.assert_matches_file(opts.output_file);
  output.assert_exit_code(opts.exit_code);
}

#[test]
fn compile_node_modules_symlink_outside() {
  let context = TestContextBuilder::for_npm()
    .use_sync_npm_download()
    .use_copy_temp_dir("compile/node_modules_symlink_outside")
    .cwd("compile/node_modules_symlink_outside")
    .build();

  let temp_dir = context.temp_dir();
  let project_dir = temp_dir
    .path()
    .join("compile")
    .join("node_modules_symlink_outside");
  temp_dir.create_dir_all(project_dir.join("node_modules"));
  temp_dir.create_dir_all(project_dir.join("some_folder"));
  temp_dir.write(project_dir.join("test.txt"), "5");

  // create a symlink in the node_modules directory that points to a folder in the cwd
  temp_dir.symlink_dir(
    project_dir.join("some_folder"),
    project_dir.join("node_modules").join("some_folder"),
  );
  // compile folder
  let output = context
    .new_command()
    .args("compile --allow-read --node-modules-dir --output bin main.ts")
    .run();
  output.assert_exit_code(0);
  output.assert_matches_file(
    "compile/node_modules_symlink_outside/main_compile_folder.out",
  );
  assert!(project_dir.join("node_modules/some_folder").exists());

  // Cleanup and remove the folder. The folder test is done separately from
  // the file symlink test because different systems would traverse
  // the directory items in different order.
  temp_dir.remove_dir_all(project_dir.join("node_modules/some_folder"));

  // create a symlink in the node_modules directory that points to a file in the cwd
  temp_dir.symlink_file(
    project_dir.join("test.txt"),
    project_dir.join("node_modules").join("test.txt"),
  );
  assert!(project_dir.join("node_modules/test.txt").exists());

  // compile
  let output = context
    .new_command()
    .args("compile --allow-read --node-modules-dir --output bin main.ts")
    .run();
  output.assert_exit_code(0);
  output.assert_matches_file(
    "compile/node_modules_symlink_outside/main_compile_file.out",
  );

  // run
  let binary_path =
    project_dir.join(if cfg!(windows) { "bin.exe" } else { "bin" });
  let output = context.new_command().command_name(binary_path).run();
  output.assert_matches_file("compile/node_modules_symlink_outside/main.out");
}
