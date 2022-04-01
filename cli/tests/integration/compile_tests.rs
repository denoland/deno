// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fs::File;
use std::process::Command;
use test_util as util;
use test_util::TempDir;

#[test]
fn compile() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  let output = util::deno_cmd()
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
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, "Welcome to Deno!\n".as_bytes());
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
    .arg("./028_args.ts")
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
    .arg("./standalone_error.ts")
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
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert!(stderr.contains("error: Error: boom!\n    at boom (file://"));
  assert!(stderr.contains("standalone_error.ts:2:11"));
  assert!(stderr.contains("at foo (file://"));
  assert!(stderr.contains("standalone_error.ts:5:5"));
  assert!(stderr.contains("standalone_error.ts:7:1"));
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
    .arg("./standalone_error_module_with_imports_1.ts")
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
  println!("{:#?}", &output);
  assert_eq!(output.stdout, b"hello\n");
  let stderr = String::from_utf8(output.stderr).unwrap();
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert!(stderr.contains("error: Error: boom!\n    at file://"));
  assert!(stderr.contains("standalone_error_module_with_imports_2.ts:2:7"));
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
    .arg("./standalone_import_datauri.ts")
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
  let dir = TempDir::new().unwrap();
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
    .arg("./standalone_follow_redirects.ts")
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
fn standalone_compiler_ops() {
  let dir = TempDir::new();
  let exe = if cfg!(windows) {
    dir.path().join("standalone_compiler_ops.exe")
  } else {
    dir.path().join("standalone_compiler_ops")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./standalone_compiler_ops.ts")
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
  assert_eq!(output.stdout, b"Hello, Compiler API!\n");
}

#[test]
fn compile_with_directory_output_flag() {
  let dir = TempDir::new();
  let output_path = if cfg!(windows) {
    dir.path().join(r"args\random\")
  } else {
    dir.path().join("args/random/")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&output_path)
    .arg("./standalone_compiler_ops.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let exe = if cfg!(windows) {
    output_path.join("standalone_compiler_ops.exe")
  } else {
    output_path.join("standalone_compiler_ops")
  };
  assert!(&exe.exists());
  let output = Command::new(exe)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Hello, Compiler API!\n");
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
    .arg("./028_args.ts")
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
  assert!(stderr.contains(&expected_stderr));
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
    .arg("./028_args.ts")
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
  assert!(stderr.contains(&expected_stderr));
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
    .arg("./028_args.ts")
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
  dbg!(&stderr);
  assert!(stderr.contains(&expected_stderr));
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
    .arg("./028_args.ts")
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
    .arg("./028_args.ts")
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
    .arg("./standalone_runtime_flags.ts")
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
  assert!(util::strip_ansi_codes(&stderr_str)
    .contains("PermissionDenied: Requires write access"));
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
    .arg("standalone_import_map.json")
    .arg("--output")
    .arg(&exe)
    .arg("./standalone_import_map.ts")
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
    .arg("./001_hello.js")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());

  //no "Bundle testdata_path/001_hello.js" in output
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
