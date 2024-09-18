// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::process::Command;
use std::process::Stdio;
use std::time::Instant;
use test_util as util;
use test_util::assert_starts_with;
use test_util::TestContext;
use util::TestContextBuilder;

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
