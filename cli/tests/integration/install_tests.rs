// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::process::Command;
use tempfile::TempDir;
use test_util as util;

#[test]
fn install_basic() {
  let _guard = util::http_server();
  let temp_dir = TempDir::new().unwrap();
  let temp_dir_str = temp_dir.path().to_string_lossy().to_string();

  let status = util::deno_cmd()
    .current_dir(temp_dir.path())
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("http://localhost:4545/echo.ts")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", ""),
    ])
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());

  let mut file_path = temp_dir.path().join(".deno/bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = fs::read_to_string(file_path).unwrap();
  // ensure there's a trailing newline so the shell script can be
  // more versatile.
  assert_eq!(content.chars().last().unwrap(), '\n');

  if cfg!(windows) {
    assert!(content.contains(r#""run" "http://localhost:4545/echo.ts""#));
  } else {
    assert!(content.contains(r#"run 'http://localhost:4545/echo.ts'"#));
  }
}

#[test]
fn install_custom_dir_env_var() {
  let _guard = util::http_server();
  let temp_dir = TempDir::new().unwrap();
  let temp_dir_str = temp_dir.path().to_string_lossy().to_string();

  let status = util::deno_cmd()
    .current_dir(util::root_path()) // different cwd
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("http://localhost:4545/echo.ts")
    .envs([
      ("HOME", temp_dir_str.as_str()),
      ("USERPROFILE", temp_dir_str.as_str()),
      ("DENO_INSTALL_ROOT", temp_dir_str.as_str()),
    ])
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());

  let mut file_path = temp_dir.path().join("bin/echo_test");
  assert!(file_path.exists());

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let content = fs::read_to_string(file_path).unwrap();
  if cfg!(windows) {
    assert!(content.contains(r#""run" "http://localhost:4545/echo.ts""#));
  } else {
    assert!(content.contains(r#"run 'http://localhost:4545/echo.ts'"#));
  }
}

#[test]
fn installer_test_local_module_run() {
  let temp_dir = TempDir::new().unwrap();
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("--root")
    .arg(temp_dir.path())
    .arg(util::testdata_path().join("echo.ts"))
    .arg("hello")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let mut file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
  // NOTE: using file_path here instead of exec_name, because tests
  // shouldn't mess with user's PATH env variable
  let output = Command::new(file_path)
    .current_dir(temp_dir.path())
    .arg("foo")
    .env("PATH", util::target_dir())
    .output()
    .unwrap();
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout_str.ends_with("hello, foo"));
}

#[test]
fn installer_test_remote_module_run() {
  let _g = util::http_server();
  let temp_dir = TempDir::new().unwrap();
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("--root")
    .arg(temp_dir.path())
    .arg("http://localhost:4545/echo.ts")
    .arg("hello")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let mut file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
  let output = Command::new(file_path)
    .current_dir(temp_dir.path())
    .arg("foo")
    .env("PATH", util::target_dir())
    .output()
    .unwrap();
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("hello, foo"));
}
