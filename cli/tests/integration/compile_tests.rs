// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::process::Command;
use tempfile::TempDir;
use test_util as util;

#[test]
fn compile() {
  let dir = TempDir::new().expect("tempdir fail");
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

#[ignore]
#[test]
#[cfg(windows)]
// https://github.com/denoland/deno/issues/9667
fn compile_windows_ext() {
  let dir = TempDir::new().expect("tempdir fail");
  let exe = dir.path().join("welcome_9667");
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("--target")
    .arg("x86_64-unknown-linux-gnu")
    .arg("./test_util/std/examples/welcome.ts")
    // TODO(kt3k): Prints command output to the test log for debugging purpose.
    // Uncomment this line when this test become stable.
    //.stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(std::path::Path::new(&exe).exists());
}

#[test]
fn standalone_args() {
  let dir = TempDir::new().expect("tempdir fail");
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
  let dir = TempDir::new().expect("tempdir fail");
  let exe = if cfg!(windows) {
    dir.path().join("error.exe")
  } else {
    dir.path().join("error")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
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
  let expected_stderr = "error: Error: boom!\n    at boom (file://$deno$/bundle.js:2:11)\n    at foo (file://$deno$/bundle.js:5:5)\n    at file://$deno$/bundle.js:7:1\n";
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert_eq!(stderr, expected_stderr);
}

#[test]
fn standalone_no_module_load() {
  let dir = TempDir::new().expect("tempdir fail");
  let exe = if cfg!(windows) {
    dir.path().join("hello.exe")
  } else {
    dir.path().join("hello")
  };
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("compile")
    .arg("--unstable")
    .arg("--output")
    .arg(&exe)
    .arg("./standalone_import.ts")
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
  assert_eq!(output.stdout, b"start\n");
  let stderr_str = String::from_utf8(output.stderr).unwrap();
  assert!(util::strip_ansi_codes(&stderr_str)
    .contains("Self-contained binaries don't support module loading"));
}

#[test]
fn standalone_load_datauri() {
  let dir = TempDir::new().expect("tempdir fail");
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

#[test]
fn standalone_compiler_ops() {
  let dir = TempDir::new().expect("tempdir fail");
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
fn compile_with_directory_exists_error() {
  let dir = TempDir::new().expect("tempdir fail");
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::create_dir(&exe).expect("cannot create directory");
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
  let expected_stderr =
    format!("Could not compile: {:?} is a directory.\n", &exe);
  let stderr = String::from_utf8(output.stderr).unwrap();
  assert!(stderr.contains(&expected_stderr));
}

#[test]
fn compile_with_conflict_file_exists_error() {
  let dir = TempDir::new().expect("tempdir fail");
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::write(&exe, b"SHOULD NOT BE OVERWRITTEN")
    .expect("cannot create file");
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
  let expected_stderr =
    format!("Could not compile: cannot overwrite {:?}.\n", &exe);
  let stderr = String::from_utf8(output.stderr).unwrap();
  dbg!(&stderr);
  assert!(stderr.contains(&expected_stderr));
  assert!(std::fs::read(&exe)
    .expect("cannot read file")
    .eq(b"SHOULD NOT BE OVERWRITTEN"));
}

#[test]
fn compile_and_overwrite_file() {
  let dir = TempDir::new().expect("tempdir fail");
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
  let dir = TempDir::new().expect("tempdir fail");
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
