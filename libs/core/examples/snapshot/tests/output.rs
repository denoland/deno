// Copyright 2018-2025 the Deno authors. MIT license.

use core::str;
use std::process::Command;
use std::process::Output;

#[test]
fn check_output() -> Result<(), Box<dyn std::error::Error>> {
  let output = capture_output()?;

  let err = str::from_utf8(&output.stderr)?;
  assert_eq!(err, "");
  assert!(output.status.success());

  let out = str::from_utf8(&output.stdout)?;
  assert_eq!(out, "Received this value from JS: Hello from example.js\n");

  Ok(())
}

/// NOTE!: This is NOT the preferred pattern to follow for testing binary crates!
/// See: <https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests-for-binary-crates>
///
/// However, we want to keep this example simple, so we're not going to create separate main.rs & lib.rs, or
/// add injectable outputs. We'll just run the binary and then capture its output.
fn capture_output() -> Result<Output, std::io::Error> {
  Command::new("cargo")
    .args([
      "run",
      "--release", // CI runs in --release mode, so re-use its cache.
      "--quiet",   // only capture the command's output.
    ])
    .output()
}
