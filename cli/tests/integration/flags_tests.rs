// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::assert_contains;

#[test]
fn help_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--help")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn help_output() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--help")
    .run();

  let stdout = output.combined_output();
  let subcommand_descriptions = vec![
    "Run a JavaScript or TypeScript program",
    "Run benchmarks",
    "Bundle module and dependencies into single file",
    "Cache the dependencies",
    "Type-check the dependencies",
    "Compile the script into a self contained executable",
    "Generate shell completions",
    "Print coverage reports",
    "Show documentation for a module",
    "Eval script",
    "Format source files",
    "Initialize a new project",
    "Show info about cache or info related to source file",
    "Install script as an executable",
    "Uninstall a script previously installed with deno install",
    "Start the language server",
    "Lint source files",
    "Read Eval Print Loop",
    "Run a task defined in the configuration file",
    "Run tests",
    "Print runtime TypeScript declarations",
    #[cfg(feature = "upgrade")]
    "Upgrade deno executable to given version",
    "Vendor remote modules into a local directory",
    "Print this message or the help of the given subcommand(s)",
  ];

  for description in subcommand_descriptions {
    assert_contains!(stdout, description);
  }
}

#[test]
fn version_short_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("-V")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn version_long_flag() {
  let status = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--version")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

itest!(types {
  args: "types",
  output: "types/types.out",
});
