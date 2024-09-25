// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use test_util as util;
use util::assert_contains;
use util::assert_not_contains;
use util::testdata_path;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn compile_basic() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  // try this twice to ensure it works with the cache
  for _ in 0..2 {
    let output = context
      .new_command()
      .args_vec([
        "compile",
        "--output",
        &exe.to_string_lossy(),
        "../../tests/testdata/welcome.ts",
      ])
      .run();
    output.assert_exit_code(0);
    output.skip_output_check();
    let output = context.new_command().name(&exe).run();
    output.assert_matches_text("Welcome to Deno!\n");
  }

  // On arm64 macOS, check if `codesign -v` passes
  #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
  {
    let output = std::process::Command::new("codesign")
      .arg("-v")
      .arg(&exe)
      .output()
      .unwrap();
    assert!(output.status.success());
  }

  // now ensure this works when the deno_dir is readonly
  let readonly_dir = dir.path().join("readonly");
  readonly_dir.make_dir_readonly();
  let readonly_sub_dir = readonly_dir.join("sub");

  let output = context
    .new_command()
    // it should fail creating this, but still work
    .env("DENO_DIR", readonly_sub_dir)
    .name(exe)
    .run();
  output.assert_matches_text("Welcome to Deno!\n");
}

#[test]
fn standalone_args() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/args.ts",
      "a",
      "b",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .name(&exe)
    .args("foo --bar --unstable")
    .run()
    .assert_matches_text("a\nb\nfoo\n--bar\n--unstable\n")
    .assert_exit_code(0);
}

#[test]
fn standalone_error() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("error.exe")
  } else {
    dir.path().join("error")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_error.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let output = context.new_command().name(&exe).split_output().run();
  output.assert_exit_code(1);
  output.assert_stdout_matches_text("");
  let stderr = output.stderr();
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert_contains!(stderr, "error: Uncaught (in promise) Error: boom!");
  assert_contains!(stderr, "\n    at boom (file://");
  assert_contains!(stderr, "standalone_error.ts:2:9");
  assert_contains!(stderr, "at foo (file://");
  assert_contains!(stderr, "standalone_error.ts:5:3");
  assert_contains!(stderr, "standalone_error.ts:7:1");
}

#[test]
fn standalone_error_module_with_imports() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("error.exe")
  } else {
    dir.path().join("error")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_error_module_with_imports_1.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  let output = context
    .new_command()
    .name(&exe)
    .env("NO_COLOR", "1")
    .split_output()
    .run();
  output.assert_stdout_matches_text("hello\n");
  let stderr = output.stderr();
  // On Windows, we cannot assert the file path (because '\').
  // Instead we just check for relevant output.
  assert_contains!(stderr, "error: Uncaught (in promise) Error: boom!");
  assert_contains!(stderr, "\n    at file://");
  assert_contains!(stderr, "standalone_error_module_with_imports_2.ts:2:7");
  output.assert_exit_code(1);
}

#[test]
fn standalone_load_datauri() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("load_datauri.exe")
  } else {
    dir.path().join("load_datauri")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_import_datauri.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .name(&exe)
    .run()
    .assert_matches_text("Hello Deno!\n")
    .assert_exit_code(0);
}

// https://github.com/denoland/deno/issues/13704
#[test]
fn standalone_follow_redirects() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("follow_redirects.exe")
  } else {
    dir.path().join("follow_redirects")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "--config",
      "../config/deno.json",
      "./compile/standalone_follow_redirects.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .name(&exe)
    .run()
    .assert_matches_text("Hello\n")
    .assert_exit_code(0);
}

#[test]
fn compile_with_file_exists_error() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let output_path = if cfg!(windows) {
    dir.path().join(r"args\")
  } else {
    dir.path().join("args/")
  };
  let file_path = dir.path().join("args");
  file_path.write("");
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &output_path.to_string_lossy(),
      "./compile/args.ts",
    ])
    .run()
    .assert_matches_text(&format!(
      concat!(
        "[WILDCARD]error: Could not compile to file '{}' because its parent directory ",
        "is an existing file. You can use the `--output <file-path>` flag to ",
        "provide an alternative name.\n",
      ),
      file_path,
    ))
    .assert_exit_code(1);
}

#[test]
fn compile_with_directory_exists_error() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::create_dir(&exe).unwrap();
  context.new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/args.ts"
    ]).run()
    .assert_matches_text(&format!(
      concat!(
        "[WILDCARD]error: Could not compile to file '{}' because a directory exists with ",
        "the same name. You can use the `--output <file-path>` flag to ",
        "provide an alternative name.\n"
      ),
      exe
    ))
    .assert_exit_code(1);
}

#[test]
fn compile_with_conflict_file_exists_error() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };
  std::fs::write(&exe, b"SHOULD NOT BE OVERWRITTEN").unwrap();
  context.new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/args.ts"
    ]).run()
    .assert_matches_text(&format!(
      concat!(
        "[WILDCARD]error: Could not compile to file '{}' because the file already exists ",
        "and cannot be overwritten. Please delete the existing file or ",
        "use the `--output <file-path>` flag to provide an alternative name.\n"
      ),
      exe
    ))
    .assert_exit_code(1);
  exe.assert_matches_text("SHOULD NOT BE OVERWRITTEN");
}

#[test]
fn compile_and_overwrite_file() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("args.exe")
  } else {
    dir.path().join("args")
  };

  // do this twice
  for _ in 0..2 {
    context
      .new_command()
      .args_vec([
        "compile",
        "--output",
        &exe.to_string_lossy(),
        "./compile/args.ts",
      ])
      .run()
      .skip_output_check()
      .assert_exit_code(0);
    assert!(&exe.exists());
  }
}

#[test]
fn standalone_runtime_flags() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("flags.exe")
  } else {
    dir.path().join("flags")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-read",
      "--seed",
      "1",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_runtime_flags.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .env("NO_COLOR", "1")
    .name(&exe)
    .split_output()
    .run()
    .assert_stdout_matches_text("0.147205063401058\n")
    .assert_stderr_matches_text(
      "[WILDCARD]NotCapable: Requires write access to[WILDCARD]",
    )
    .assert_exit_code(1);
}

#[test]
fn standalone_ext_flag_ts() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("ext_flag_ts.exe")
  } else {
    dir.path().join("ext_flag_ts")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--ext",
      "ts",
      "--output",
      &exe.to_string_lossy(),
      "./file_extensions/ts_without_extension",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .env("NO_COLOR", "1")
    .name(&exe)
    .run()
    .assert_matches_text("executing typescript with no extension\n")
    .assert_exit_code(0);
}

#[test]
fn standalone_ext_flag_js() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("ext_flag_js.exe")
  } else {
    dir.path().join("ext_flag_js")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--ext",
      "js",
      "--output",
      &exe.to_string_lossy(),
      "./file_extensions/js_without_extension",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .env("NO_COLOR", "1")
    .name(&exe)
    .run()
    .assert_matches_text("executing javascript with no extension\n");
}

#[test]
fn standalone_import_map() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("import_map.exe")
  } else {
    dir.path().join("import_map")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-read",
      "--import-map",
      "compile/standalone_import_map.json",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_import_map.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .name(&exe)
    .run()
    .skip_output_check()
    .assert_exit_code(0);
}

#[test]
fn standalone_import_map_config_file() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("import_map.exe")
  } else {
    dir.path().join("import_map")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-read",
      "--config",
      "compile/standalone_import_map_config.json",
      "--output",
      &exe.to_string_lossy(),
      "./compile/standalone_import_map.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  context
    .new_command()
    .name(&exe)
    .run()
    .skip_output_check()
    .assert_exit_code(0);
}

#[test]
// https://github.com/denoland/deno/issues/12670
fn skip_rebundle() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("hello_world.exe")
  } else {
    dir.path().join("hello_world")
  };
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./run/001_hello.js",
    ])
    .run();

  //no "Bundle testdata_path/run/001_hello.js" in output
  assert_not_contains!(output.combined_output(), "Bundle");

  context
    .new_command()
    .name(&exe)
    .run()
    .assert_matches_text("Hello World\n")
    .assert_exit_code(0);
}

#[test]
fn check_local_by_default() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-import",
      "--output",
      &exe.to_string_lossy(),
      "./compile/check_local_by_default.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
}

#[test]
fn check_local_by_default2() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("welcome.exe")
  } else {
    dir.path().join("welcome")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-import",
      "--output",
      &exe.to_string_lossy(),
      "./compile/check_local_by_default2.ts"
    ])
    .run()
    .assert_matches_text(
      r#"[WILDCARD]error: TS2322 [ERROR]: Type '12' is not assignable to type '"b"'.[WILDCARD]"#,
    )
    .assert_exit_code(1);
}

#[test]
fn workers_basic() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("basic.exe")
  } else {
    dir.path().join("basic")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--no-check",
      "--output",
      &exe.to_string_lossy(),
      "./compile/workers/basic.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  context
    .new_command()
    .name(&exe)
    .run()
    .assert_matches_file("./compile/workers/basic.out")
    .assert_exit_code(0);
}

#[test]
fn workers_not_in_module_map() {
  let context = TestContext::with_http_server();
  let temp_dir = context.temp_dir();
  let exe = if cfg!(windows) {
    temp_dir.path().join("not_in_module_map.exe")
  } else {
    temp_dir.path().join("not_in_module_map")
  };
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/workers/not_in_module_map.ts",
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();

  let output = context.new_command().name(exe).env("NO_COLOR", "").run();
  output.assert_exit_code(1);
  output.assert_matches_text(concat!(
    "error: Uncaught (in worker \"\") Module not found: [WILDCARD]",
    "error: Uncaught (in promise) Error: Unhandled error in child worker.\n[WILDCARD]"
  ));
}

#[test]
fn workers_with_include_flag() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("workers_with_include_flag.exe")
  } else {
    dir.path().join("workers_with_include_flag")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "--include",
      "./compile/workers/worker.ts",
      "./compile/workers/not_in_module_map.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  context
    .new_command()
    .name(&exe)
    .env("NO_COLOR", "")
    .run()
    .assert_matches_text("Hello from worker!\nReceived 42\nClosing\n");
}

#[test]
fn dynamic_import() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("dynamic_import.exe")
  } else {
    dir.path().join("dynamic_import")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/dynamic_imports/main.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  context
    .new_command()
    .name(&exe)
    .env("NO_COLOR", "")
    .run()
    .assert_matches_file("./compile/dynamic_imports/main.out")
    .assert_exit_code(0);
}

#[test]
fn dynamic_import_unanalyzable() {
  let context = TestContext::with_http_server();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("dynamic_import_unanalyzable.exe")
  } else {
    dir.path().join("dynamic_import_unanalyzable")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-read",
      "--include",
      "./compile/dynamic_imports/import1.ts",
      "--output",
      &exe.to_string_lossy(),
      "./compile/dynamic_imports/main_unanalyzable.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);

  context
    .new_command()
    .current_dir(util::root_path())
    .name(&exe)
    .env("NO_COLOR", "")
    .run()
    .assert_matches_file("./compile/dynamic_imports/main.out")
    .assert_exit_code(0);
}

// TODO(2.0): this test should first run `deno install`?
#[test]
#[ignore]
fn compile_npm_specifiers() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();

  let temp_dir = context.temp_dir();
  temp_dir.write(
    "main.ts",
    concat!(
      "import path from 'node:path';\n",
      "import { getValue, setValue } from 'npm:@denotest/esm-basic';\n",
      "import getValueDefault from 'npm:@denotest/esm-import-cjs-default';\n",
      "setValue(2);\n",
      "console.log(path.join('testing', 'this'));",
      "console.log(getValue());",
      "console.log(getValueDefault());",
    ),
  );

  let binary_path = if cfg!(windows) {
    temp_dir.path().join("binary.exe")
  } else {
    temp_dir.path().join("binary")
  };

  // try with and without --node-modules-dir
  let compile_commands = &[
    "compile --output binary main.ts",
    "compile --node-modules-dir --output binary main.ts",
  ];

  for compile_command in compile_commands {
    let output = context.new_command().args(compile_command).run();
    output.assert_exit_code(0);
    output.skip_output_check();

    let output = context.new_command().name(&binary_path).run();
    output.assert_matches_text(
      r#"Node esm importing node cjs
===========================
{
  default: [Function (anonymous)],
  named: [Function (anonymous)],
  MyClass: [class MyClass]
}
{ default: [Function (anonymous)], named: [Function (anonymous)] }
[Module: null prototype] {
  MyClass: [class MyClass],
  __esModule: true,
  default: {
    default: [Function (anonymous)],
    named: [Function (anonymous)],
    MyClass: [class MyClass]
  },
  named: [Function (anonymous)]
}
[Module: null prototype] {
  __esModule: true,
  default: { default: [Function (anonymous)], named: [Function (anonymous)] },
  named: [Function (anonymous)]
}
===========================
static method
testing[WILDCARD]this
2
5
"#,
    );
  }

  // try with a package.json
  temp_dir.remove_dir_all("node_modules");
  temp_dir.write(
    "main.ts",
    concat!(
      "import { getValue, setValue } from '@denotest/esm-basic';\n",
      "setValue(2);\n",
      "console.log(getValue());",
    ),
  );
  temp_dir.write(
    "package.json",
    r#"{ "dependencies": { "@denotest/esm-basic": "1" } }"#,
  );

  context
    .new_command()
    .args("compile --output binary main.ts")
    .run()
    .assert_exit_code(0)
    .skip_output_check();

  context
    .new_command()
    .name(&binary_path)
    .run()
    .assert_matches_text("2\n");

  // now try with byonm
  temp_dir.remove_dir_all("node_modules");
  temp_dir.write("deno.json", r#"{"unstable":["byonm"]}"#);
  context.run_npm("install");

  context
    .new_command()
    .args("compile --output binary main.ts")
    .run()
    .assert_exit_code(0)
    .assert_matches_text("Check file:///[WILDLINE]/main.ts\nCompile file:///[WILDLINE]/main.ts to binary[WILDLINE]\n");

  context
    .new_command()
    .name(&binary_path)
    .run()
    .assert_matches_text("2\n");
}

#[test]
fn compile_npm_file_system() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "compile/npm_fs/main.ts",
    copy_temp_dir: Some("compile/npm_fs"),
    compile_args: vec!["-A"],
    run_args: vec![],
    output_file: "compile/npm_fs/main.out",
    node_modules_local: true,
    input_name: Some("binary"),
    expected_name: "binary",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_bin_esm() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:@denotest/bin/cli-esm",
    copy_temp_dir: None,
    compile_args: vec![],
    run_args: vec!["this", "is", "a", "test"],
    output_file: "npm/deno_run_esm.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cli-esm",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_bin_cjs() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:@denotest/bin/cli-cjs",
    copy_temp_dir: None,
    compile_args: vec![],
    run_args: vec!["this", "is", "a", "test"],
    output_file: "npm/deno_run_cjs.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cli-cjs",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_cowsay_main() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0",
    copy_temp_dir: None,
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_vfs_implicit_read_permissions() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "compile/vfs_implicit_read_permission/main.ts",
    copy_temp_dir: Some("compile/vfs_implicit_read_permission"),
    compile_args: vec![],
    run_args: vec![],
    output_file: "compile/vfs_implicit_read_permission/main.out",
    node_modules_local: false,
    input_name: Some("binary"),
    expected_name: "binary",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_no_permissions() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0",
    copy_temp_dir: None,
    compile_args: vec![],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay_no_permissions.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 1,
  });
}

#[test]
fn compile_npm_cowsay_explicit() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0/cowsay",
    copy_temp_dir: None,
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowsay.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cowsay",
    exit_code: 0,
  });
}

#[test]
fn compile_npm_cowthink() {
  run_npm_bin_compile_test(RunNpmBinCompileOptions {
    input_specifier: "npm:cowsay@1.5.0/cowthink",
    copy_temp_dir: None,
    compile_args: vec!["--allow-read"],
    run_args: vec!["Hello"],
    output_file: "npm/deno_run_cowthink.out",
    node_modules_local: false,
    input_name: None,
    expected_name: "cowthink",
    exit_code: 0,
  });
}

struct RunNpmBinCompileOptions<'a> {
  input_specifier: &'a str,
  copy_temp_dir: Option<&'a str>,
  node_modules_local: bool,
  output_file: &'a str,
  input_name: Option<&'a str>,
  expected_name: &'a str,
  run_args: Vec<&'a str>,
  compile_args: Vec<&'a str>,
  exit_code: i32,
}

fn run_npm_bin_compile_test(opts: RunNpmBinCompileOptions) {
  let builder = TestContextBuilder::for_npm();
  let context = match opts.copy_temp_dir {
    Some(copy_temp_dir) => builder.use_copy_temp_dir(copy_temp_dir).build(),
    None => builder.use_temp_cwd().build(),
  };

  let temp_dir = context.temp_dir();
  let mut args = vec!["compile".to_string()];

  args.extend(opts.compile_args.iter().map(|s| s.to_string()));

  if opts.node_modules_local {
    args.push("--node-modules-dir=auto".to_string());
  }

  if let Some(bin_name) = opts.input_name {
    args.push("--output".to_string());
    args.push(bin_name.to_string());
  }

  args.push(opts.input_specifier.to_string());

  // compile
  let output = context.new_command().args_vec(args).run();
  output.assert_exit_code(0);
  output.skip_output_check();

  // delete the npm folder in the DENO_DIR to ensure it's not using it
  context.deno_dir().remove_dir_all("./npm");

  // run
  let binary_path = if cfg!(windows) {
    temp_dir.path().join(format!("{}.exe", opts.expected_name))
  } else {
    temp_dir.path().join(opts.expected_name)
  };
  let output = context
    .new_command()
    .name(binary_path)
    .args_vec(opts.run_args)
    .run();
  output.assert_matches_file(opts.output_file);
  output.assert_exit_code(opts.exit_code);
}

#[test]
fn compile_node_modules_symlink_outside() {
  // this code is using a canonicalized temp dir because otherwise
  // it fails on the Windows CI because Deno makes the root directory
  // a common ancestor of the symlinked temp dir and the canonicalized
  // temp dir, which causes the warnings to not be surfaced
  #[allow(deprecated)]
  let context = TestContextBuilder::for_npm()
    .use_canonicalized_temp_dir()
    .use_copy_temp_dir("compile/node_modules_symlink_outside")
    .cwd("compile/node_modules_symlink_outside")
    .build();

  let temp_dir = context.temp_dir();
  let project_dir = temp_dir
    .path()
    .join("compile")
    .join("node_modules_symlink_outside");
  let symlink_target_dir = temp_dir.path().join("some_folder");
  project_dir.join("node_modules").create_dir_all();
  symlink_target_dir.create_dir_all();
  let symlink_target_file = temp_dir.path().join("target.txt");
  symlink_target_file.write("5");
  let symlink_dir = project_dir.join("node_modules").join("symlink_dir");

  // create a symlink in the node_modules directory that points to a folder outside the project
  temp_dir.symlink_dir(&symlink_target_dir, &symlink_dir);
  // compile folder
  let output = context
    .new_command()
    .args("compile --allow-read --node-modules-dir=auto --output bin main.ts")
    .run();
  output.assert_exit_code(0);
  output.assert_matches_file(
    "compile/node_modules_symlink_outside/main_compile_folder.out",
  );
  assert!(symlink_dir.exists());

  // Cleanup and remove the folder. The folder test is done separately from
  // the file symlink test because different systems would traverse
  // the directory items in different order.
  symlink_dir.remove_dir_all();

  // create a symlink in the node_modules directory that points to a file in the cwd
  temp_dir.symlink_file(
    &symlink_target_file,
    project_dir.join("node_modules").join("test.txt"),
  );
  assert!(project_dir.join("node_modules/test.txt").exists());

  // compile
  let output = context
    .new_command()
    .args("compile --allow-read --node-modules-dir=auto --output bin main.ts")
    .run();
  output.assert_exit_code(0);
  output.assert_matches_file(
    "compile/node_modules_symlink_outside/main_compile_file.out",
  );

  // run
  let binary_path =
    project_dir.join(if cfg!(windows) { "bin.exe" } else { "bin" });
  let output = context.new_command().name(binary_path).run();
  output.assert_matches_file("compile/node_modules_symlink_outside/main.out");
}

#[test]
fn compile_node_modules_symlink_non_existent() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir().path();
  temp_dir.join("main.ts").write(
    r#"import { getValue, setValue } from "npm:@denotest/esm-basic";
setValue(4);
console.log(getValue());"#,
  );
  let node_modules_dir = temp_dir.join("node_modules");
  node_modules_dir.create_dir_all();
  // create a symlink that points to a non_existent file
  node_modules_dir.symlink_dir("non_existent", "folder");
  // compile folder
  let output = context
    .new_command()
    .args("compile --allow-read --node-modules-dir=auto --output bin main.ts")
    .run();
  output.assert_exit_code(0);
  output.assert_matches_text(
    r#"Download http://localhost:4260/@denotest/esm-basic
Download http://localhost:4260/@denotest/esm-basic/1.0.0.tgz
Initialize @denotest/esm-basic@1.0.0
Check file:///[WILDCARD]/main.ts
Compile file:///[WILDCARD]/main.ts to [WILDCARD]
Warning Failed resolving symlink. Ignoring.
    Path: [WILDCARD]
    Message: [WILDCARD])
"#,
  );

  // run
  let binary_path =
    temp_dir.join(if cfg!(windows) { "bin.exe" } else { "bin" });
  let output = context.new_command().name(binary_path).run();
  output.assert_matches_text("4\n");
}

#[test]
fn dynamic_imports_tmp_lit() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("app.exe")
  } else {
    dir.path().join("app")
  };
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/dynamic_imports_tmp_lit/main.js",
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();
  let output = context.new_command().name(&exe).run();
  output.assert_matches_text("a\nb\n{ data: 5 }\n{ data: 1 }\n");
}

#[test]
fn granular_unstable_features() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("app.exe")
  } else {
    dir.path().join("app")
  };
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "--unstable-kv",
      "--unstable-temporal",
      "./compile/unstable_features.ts",
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();
  let output = context.new_command().name(&exe).run();
  output.assert_exit_code(0);
  output.assert_matches_text("Kv {}\nObject [Temporal] {}\n");
}

#[test]
fn granular_unstable_features_config_file() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let dir = context.temp_dir();
  testdata_path()
    .join("compile/unstable_features.ts")
    .copy(&dir.path().join("unstable_features.ts"));
  let exe = if cfg!(windows) {
    dir.path().join("app.exe")
  } else {
    dir.path().join("app")
  };
  dir.write(
    "deno.json",
    serde_json::to_string_pretty(&serde_json::json!({
      "unstable": ["kv", "temporal"]
    }))
    .unwrap(),
  );
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--config",
      &dir.path().join("deno.json").to_string(),
      "--output",
      &exe.to_string_lossy(),
      "./unstable_features.ts",
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();
  let output = context.new_command().name(&exe).run();
  output.assert_exit_code(0);
  output.assert_matches_text("Kv {}\nObject [Temporal] {}\n");
}

#[test]
fn dynamic_import_bad_data_uri() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("app.exe")
  } else {
    dir.path().join("app")
  };
  let file = dir.path().join("bad_data_uri.ts");
  file.write("await import('data:application/')");
  let output = context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      &file.to_string_lossy(),
    ])
    .run();
  output.assert_exit_code(0);
  output.skip_output_check();
  let output = context.new_command().name(&exe).run();
  output.assert_exit_code(1);
  output.assert_matches_text(
    "[WILDCARD]TypeError: Unable to decode data url.[WILDCARD]",
  );
}

#[test]
fn standalone_config_file_respects_compiler_options() {
  let context = TestContextBuilder::new().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("compiler_options.exe")
  } else {
    dir.path().join("compiler_options")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--allow-read",
      "--config",
      "compile/compiler_options/deno.json",
      "--output",
      &exe.to_string_lossy(),
      "./compile/compiler_options/main.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  let output = context.new_command().name(&exe).run();

  output.assert_exit_code(0);
  output.assert_matches_text("[WILDCARD]C.test() called[WILDCARD]");
}

#[test]
fn standalone_jsr_dynamic_import() {
  let context = TestContextBuilder::for_jsr().build();
  let dir = context.temp_dir();
  let exe = if cfg!(windows) {
    dir.path().join("jsr_dynamic_import.exe")
  } else {
    dir.path().join("jsr_dynamic_import")
  };
  context
    .new_command()
    .args_vec([
      "compile",
      "--output",
      &exe.to_string_lossy(),
      "./compile/jsr_dynamic_import/main.ts",
    ])
    .run()
    .skip_output_check()
    .assert_exit_code(0);
  let output = context.new_command().name(&exe).run();

  output.assert_exit_code(0);
  output.assert_matches_text("Hello world\n");
}
