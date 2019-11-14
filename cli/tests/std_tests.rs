// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
extern crate lazy_static;
extern crate tempfile;
use deno_cli::test_util::*;
use std::process::Command;
use std::env;
use tempfile::TempDir;

#[test]
fn std_tests() {
  // TODO: this is hacky and should conditionally skipped
  // some more idiomatic way
  // Run only with release build
  match env::var("DENO_BUILD_MODE") {
    Ok(mode) => {
      if mode != "release" {
        return;
      }
    }
    None => {
      return;
    }
  }

  let dir = TempDir::new().expect("tempdir fail");
  let mut deno_cmd = Command::new(deno_exe_path());
  deno_cmd.env("DENO_DIR", dir.path());

  let mut cwd = root_path();
  cwd.push("std");
  let mut deno = deno_cmd
    .current_dir(cwd) // TODO: std tests expect to run from "std" dir
    .arg("-A")
    .arg("./testing/runner.ts")
    .arg("--exclude=testing/testdata")
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}
