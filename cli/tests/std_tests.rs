// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[allow(clippy::write_with_newline)]
mod tests {
  extern crate lazy_static;
  extern crate tempfile;
  use deno::test_util::*;
  use std;
  use std::io::Write;
  use std::process::Command;
  use std::time::Instant;
  use tempfile::TempDir;

  #[test]
  fn std_tests() {
    let dir = TempDir::new().expect("tempdir fail");
    let mut deno_cmd = Command::new(deno_exe_path());
    deno_cmd.env("DENO_DIR", dir.path());

    let mut cwd = root_path();
    cwd.push("std");
    let start = Instant::now();
    let mut deno = deno_cmd
      .current_dir(cwd) // note: std tests expect to run from "std" dir
      .arg("test")
      .arg("--seed=86") // Some tests rely on specific random numbers.
      .arg("-A")
      // .arg("-Ldebug")
      .spawn()
      .expect("failed to spawn script");
    let status = deno.wait().expect("failed to wait for the child process");
    assert!(status.success());

    let duration = Instant::now() - start;
    writeln!(
      std::io::stderr(),
      "std no type check test took {:#?}",
      duration
    )
    .unwrap();
  }
}
