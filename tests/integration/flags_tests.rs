// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use util::assert_contains;

#[test]
fn help_output() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("--help")
    .run();

  let stdout = output.combined_output();
  let subcommand_descriptions = vec![
    "Run a JavaScript or TypeScript program, or a task",
    "Run a server",
    "Run a task defined in the configuration file",
    "Start an interactive Read-Eval-Print Loop (REPL) for Deno",
    "Evaluate a script from the command line",
    "Add dependencies",
    "Install script as an executable",
    "Uninstall a script previously installed with deno install",
    "Run benchmarks",
    "Type-check the dependencies",
    "Compile the script into a self contained executable",
    "Print coverage reports",
    "Genereate and show documentation for a module or built-ins",
    "Format source files",
    "Show info about cache or info related to source file",
    "Deno kernel for Jupyter notebooks",
    "Lint source files",
    "Initialize a new project",
    "Run tests",
    "Publish the current working directory's package or workspace",
    #[cfg(feature = "upgrade")]
    "Upgrade deno executable to given version",
  ];

  for description in subcommand_descriptions {
    assert_contains!(stdout, description);
  }
}
