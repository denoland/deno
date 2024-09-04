// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::process::Command;
use std::process::Stdio;
use std::time::Instant;
use test_util as util;
use test_util::assert_starts_with;
use test_util::TempDir;
use test_util::TestContext;
use util::TestContextBuilder;

// Warning: this test requires internet access.
// TODO(#7412): reenable. test is flaky
#[test]
#[ignore]
fn upgrade_in_tmpdir() {
  let temp_dir = TempDir::new();
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
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
  let temp_dir = TempDir::new_with_prefix("directory with spaces");
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
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
  let temp_dir = TempDir::new();
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
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
  let temp_dir = TempDir::new();
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
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
  let temp_dir = TempDir::new();
  let exe_path = temp_dir.path().join("deno");
  let new_exe_path = temp_dir.path().join("foo");
  util::deno_exe_path().copy(&exe_path);
  assert!(exe_path.exists());
  let mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--version")
    .arg("1.11.5")
    .arg("--output")
    .arg(&new_exe_path)
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

#[flaky_test::flaky_test]
fn upgrade_invalid_stable_version() {
  let context = upgrade_context();
  let temp_dir = context.temp_dir();
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
  assert!(exe_path.exists());
  exe_path.mark_executable();
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
  assert_starts_with!(
    &util::strip_ansi_codes(&String::from_utf8(output.stderr.clone()).unwrap())
      .to_string(),
    "error: Invalid version passed (foobar)"
  );
}

#[flaky_test::flaky_test]
fn upgrade_invalid_canary_version() {
  let context = upgrade_context();
  let temp_dir = context.temp_dir();
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
  assert!(exe_path.exists());
  exe_path.mark_executable();
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
  assert_starts_with!(
    &util::strip_ansi_codes(&String::from_utf8(output.stderr.clone()).unwrap())
      .to_string(),
    "error: Invalid commit hash passed (foobar)"
  );
}

#[flaky_test::flaky_test]
fn upgrade_invalid_lockfile() {
  let context = upgrade_context();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.deno", r#"{ \"lock\": true }"#);
  temp_dir.write(
    "deno.lock",
    r#"{
  "version": "invalid",
}"#,
  );
  let exe_path = temp_dir.path().join("deno");
  util::deno_exe_path().copy(&exe_path);
  assert!(exe_path.exists());
  exe_path.mark_executable();
  let output = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--version")
    .arg("foobar")
    .arg("--dry-run")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  // should make it here instead of erroring on an invalid lockfile
  assert_starts_with!(
    &util::strip_ansi_codes(&String::from_utf8(output.stderr.clone()).unwrap())
      .to_string(),
    "error: Invalid version passed (foobar)"
  );
}

#[flaky_test::flaky_test]
fn upgrade_prompt() {
  let context = upgrade_context();
  let temp_dir = context.temp_dir();
  // start a task that goes indefinitely in order to allow
  // the upgrade check to occur
  temp_dir.write("main.js", "setInterval(() => {}, 1_000)");
  let cmd = context
    .new_command()
    .args("run --log-level=debug main.js")
    .env_remove("DENO_NO_UPDATE_CHECK");
  // run once and wait for the version to be stored
  cmd.with_pty(|mut pty| {
    pty.expect("Finished upgrade checker.");
  });
  // now check that the upgrade prompt is shown the next time this is run
  temp_dir.write("main.js", "");
  cmd.with_pty(|mut pty| {
    // - We need to use a pty here because the upgrade prompt
    //   doesn't occur except when there's a pty.
    // - Version comes from the test server.
    pty.expect_any(&[
      " 99999.99.99 Run `deno upgrade` to install it.",
      // it builds canary releases on main, so check for this in that case
      "Run `deno upgrade canary` to install it.",
    ]);
  });
}

#[test]
fn upgrade_lsp_repl_sleeps() {
  let context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .env(
      "DENO_DONT_USE_INTERNAL_BASE_UPGRADE_URL",
      "http://localhost:4545/upgrade/sleep",
    )
    .build();
  let start_instant = Instant::now();
  // ensure this works even though the upgrade check is taking
  // a long time to complete
  context
    .new_command()
    .args("repl")
    .env_remove("DENO_NO_UPDATE_CHECK")
    .with_pty(|mut pty| {
      pty.write_line("123 + 456\n");
      pty.expect("579");
    });

  // the test server will sleep for 95 seconds, so ensure this is less
  let elapsed_secs = start_instant.elapsed().as_secs();
  assert!(elapsed_secs < 94, "elapsed_secs: {}", elapsed_secs);
}

fn upgrade_context() -> TestContext {
  TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .env(
      "DENO_DONT_USE_INTERNAL_BASE_UPGRADE_URL",
      "http://localhost:4545",
    )
    .build()
}
