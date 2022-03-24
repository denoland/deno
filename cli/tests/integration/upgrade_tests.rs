// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::process::{Command, Stdio};
use tempfile::TempDir;
use test_util as util;

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let _mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let _mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  // TODO(ry) assert!(mtime1 < mtime2);
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_with_space_in_path() {
  let temp_dir = tempfile::Builder::new()
    .prefix("directory with spaces")
    .tempdir()
    .unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .env("TMP", temp_dir.path())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_with_version_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let _mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .arg("--version")
    .arg("1.11.5")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let upgraded_deno_version = String::from_utf8(
    Command::new(&exe_path).arg("-V").output().unwrap().stdout,
  )
  .unwrap();
  assert!(upgraded_deno_version.contains("1.11.5"));
  let _mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  // TODO(ry) assert!(mtime1 < mtime2);
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_with_canary_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let _mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--canary")
    .arg("--version")
    .arg("e6685f0f01b8a11a5eaff020f5babcfde76b3038")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let upgraded_deno_version = String::from_utf8(
    Command::new(&exe_path).arg("-V").output().unwrap().stdout,
  )
  .unwrap();
  assert!(upgraded_deno_version.contains("e6685f0"));
  let _mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  // TODO(ry) assert!(mtime1 < mtime2);
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_with_out_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let new_exe_path = temp_dir.path().join("foo");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--version")
    .arg("1.11.5")
    .arg("--output")
    .arg(&new_exe_path.to_str().unwrap())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  assert!(new_exe_path.exists());
  let mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  assert_eq!(mtime1, mtime2); // Original exe_path was not changed.

  let v = String::from_utf8(
    Command::new(&new_exe_path)
      .arg("-V")
      .output()
      .unwrap()
      .stdout,
  )
  .unwrap();
  assert!(v.contains("1.11.5"));
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_invalid_stable_version() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let output = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--version")
    .arg("foobar")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(
    "error: Invalid semver passed\n",
    util::strip_ansi_codes(&String::from_utf8(output.stderr).unwrap())
  );
}

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_invalid_canary_version() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let output = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--canary")
    .arg("--version")
    .arg("foobar")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(
    "error: Invalid commit hash passed\n",
    util::strip_ansi_codes(&String::from_utf8(output.stderr).unwrap())
  );
}
