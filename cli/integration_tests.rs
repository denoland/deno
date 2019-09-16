// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::colors::strip_ansi_codes;
use os_pipe::pipe;
use std::env;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

macro_rules! itest(
  ($name:ident {$( $key:ident: $value:expr,)*})  => {
    #[test]
    fn $name() {
      (IntegrationTest {
        $(
          $key: $value,
         )*
        .. Default::default()
      }).run()
    }
  }
);

itest!(_001_hello {
  args: "run --reload 001_hello.js",
  output: "tests/001_hello.js.out",
});

itest!(_002_hello {
  args: "run --reload 002_hello.ts",
  output: "tests/002_hello.ts.out",
});

itest!(_003_relative_import {
  args: "run --reload 003_relative_import.ts",
  output: "tests/003_relative_import.ts.out",
});

itest!(_004_set_timeout {
  args: "run --reload 004_set_timeout.ts",
  output: "tests/004_set_timeout.ts.out",
});

itest!(_005_more_imports {
  args: "run --reload 005_more_imports.ts",
  output: "tests/005_more_imports.ts.out",
});

itest!(_006_url_imports {
  args: "run --reload 006_url_imports.ts",
  output: "tests/006_url_imports.ts.out",
});

itest!(_012_async {
  args: "run --reload 012_async.ts",
  output: "tests/012_async.ts.out",
});

itest!(_013_dynamic_import {
  args: "013_dynamic_import.ts --reload --allow-read",
  output: "tests/013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "014_duplicate_import.ts --reload --allow-read",
  output: "tests/014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args: "015_duplicate_parallel_import.js --reload --allow-read",
  output: "tests/015_duplicate_parallel_import.js.out",
});

itest!(_016_double_await {
  args: "run --allow-read --reload 016_double_await.ts",
  output: "tests/016_double_await.ts.out",
});

itest!(_017_import_redirect {
  args: "run --reload 017_import_redirect.ts",
  output: "tests/017_import_redirect.ts.out",
});

itest!(_018_async_catch {
  args: "run --reload 018_async_catch.ts",
  output: "tests/018_async_catch.ts.out",
});

itest!(_019_media_types {
  args: "run --reload 019_media_types.ts",
  output: "tests/019_media_types.ts.out",
});

itest!(_020_json_modules {
  args: "run --reload 020_json_modules.ts",
  output: "tests/020_json_modules.ts.out",
});

itest!(_021_mjs_modules {
  args: "run --reload 021_mjs_modules.ts",
  output: "tests/021_mjs_modules.ts.out",
});

itest!(_022_info_flag_script {
  args: "info http://127.0.0.1:4545/tests/019_media_types.ts",
  output: "tests/022_info_flag_script.out",
});

itest!(_023_no_ext_with_headers {
  args: "run --reload 023_no_ext_with_headers",
  output: "tests/023_no_ext_with_headers.out",
});

// FIXME(bartlomieju): this test should use remote file
// itest!(_024_import_no_ext_with_headers {
//   args: "run --reload 024_import_no_ext_with_headers.ts",
//   output: "tests/024_import_no_ext_with_headers.ts.out",
// });

itest!(_025_hrtime {
  args: "run --allow-hrtime --reload 025_hrtime.ts",
  output: "tests/025_hrtime.ts.out",
});

itest!(_025_reload_js_type_error {
  args: "run --reload 025_reload_js_type_error.js",
  output: "tests/025_reload_js_type_error.js.out",
});

itest!(_026_redirect_javascript {
  args: "run --reload 026_redirect_javascript.js",
  output: "tests/026_redirect_javascript.js.out",
});

itest!(_026_workers {
  args: "run --reload 026_workers.ts",
  output: "tests/026_workers.ts.out",
});

itest!(_027_redirect_typescript {
  args: "run --reload 027_redirect_typescript.ts",
  output: "tests/027_redirect_typescript.ts.out",
});

itest!(_028_args {
  args: "run --reload 028_args.ts --arg1 val1 --arg2=val2 -- arg3 arg4",
  output: "tests/028_args.ts.out",
});

itest!(_029_eval {
  args: "eval console.log(\"hello\")",
  output: "tests/029_eval.out",
});

itest!(_030_xeval {
  args: "xeval console.log($.toUpperCase())",
  input: Some("a\nb\n\nc"),
  output: "tests/030_xeval.out",
});

itest!(_031_xeval_replvar {
  args: "xeval -I val console.log(val.toUpperCase());",
  input: Some("a\nb\n\nc"),
  output: "tests/031_xeval_replvar.out",
});

itest!(_032_xeval_delim {
  args: "xeval -d DELIM console.log($.toUpperCase());",
  input: Some("aDELIMbDELIMDELIMc"),
  output: "tests/032_xeval_delim.out",
});

itest!(_033_import_map {
  args:
    "run --reload --importmap=importmaps/import_map.json importmaps/test.ts",
  output: "tests/033_import_map.out",
});

itest!(_034_onload {
  args: "run --reload 034_onload/main.ts",
  output: "tests/034_onload.out",
});

itest!(_035_no_fetch_flag {
  args: "--reload --no-fetch http://127.0.0.1:4545/tests/019_media_types.ts",
  output: "tests/035_no_fetch_flag.out",
  exit_code: 1,
  check_stderr: true,
});

itest!(_036_import_map_fetch {
  args:
    "fetch --reload --importmap=importmaps/import_map.json importmaps/test.ts",
  output: "tests/036_import_map_fetch.out",
});

itest!(_037_current_thread {
  args: "run --current-thread --reload 034_onload/main.ts",
  output: "tests/034_onload.out",
});

itest!(_038_checkjs {
  // checking if JS file is run through TS compiler
  args: "run --reload --config 038_checkjs.tsconfig.json 038_checkjs.js",
  check_stderr: true,
  exit_code: 1,
  output: "tests/038_checkjs.js.out",
});

itest!(_039_worker_deno_ns {
  args: "run --reload 039_worker_deno_ns.ts",
  output: "tests/039_worker_deno_ns.ts.out",
});

itest!(_040_worker_blob {
  args: "run --reload 040_worker_blob.ts",
  output: "tests/040_worker_blob.ts.out",
});

itest!(_041_dyn_import_eval {
  args: "eval import('./subdir/mod4.js').then(console.log)",
  output: "tests/041_dyn_import_eval.out",
});

itest!(_041_info_flag {
  args: "info",
  output: "tests/041_info_flag.out",
});

itest!(_042_dyn_import_evalcontext {
  args: "run --allow-read --reload 042_dyn_import_evalcontext.ts",
  output: "tests/042_dyn_import_evalcontext.ts.out",
});

itest!(async_error {
  exit_code: 1,
  args: "run --reload async_error.ts",
  check_stderr: true,
  output: "tests/async_error.ts.out",
});

itest!(circular1 {
  args: "run --reload circular1.js",
  output: "tests/circular1.js.out",
});

itest!(config {
  args: "run --reload --config config.tsconfig.json config.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/config.ts.out",
});

itest!(error_001 {
  args: "run --reload error_001.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_001.ts.out",
});

itest!(error_002 {
  args: "run --reload error_002.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_002.ts.out",
});

itest!(error_003_typescript {
  args: "run --reload error_003_typescript.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_003_typescript.ts.out",
});

// Supposing that we've already attempted to run error_003_typescript.ts
// we want to make sure that JS wasn't emitted. Running again without reload flag
// should result in the same output.
// https://github.com/denoland/deno/issues/2436
itest!(error_003_typescript2 {
  args: "run error_003_typescript.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_003_typescript.ts.out",
});

itest!(error_004_missing_module {
  args: "run --reload error_004_missing_module.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_004_missing_module.ts.out",
});

itest!(error_005_missing_dynamic_import {
  args: "run --reload error_005_missing_dynamic_import.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_005_missing_dynamic_import.ts.out",
});

itest!(error_006_import_ext_failure {
  args: "run --reload error_006_import_ext_failure.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_006_import_ext_failure.ts.out",
});

itest!(error_007_any {
  args: "run --reload error_007_any.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_007_any.ts.out",
});

itest!(error_008_checkjs {
  args: "run --reload error_008_checkjs.js",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_008_checkjs.js.out",
});

itest!(error_011_bad_module_specifier {
  args: "run --reload error_011_bad_module_specifier.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_011_bad_module_specifier.ts.out",
});

itest!(error_012_bad_dynamic_import_specifier {
  args: "run --reload error_012_bad_dynamic_import_specifier.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_012_bad_dynamic_import_specifier.ts.out",
});

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_013_missing_script.out",
});

itest!(error_014_catch_dynamic_import_error {
  args: "error_014_catch_dynamic_import_error.js --reload --allow-read",
  output: "tests/error_014_catch_dynamic_import_error.js.out",
});

itest!(error_015_dynamic_import_permissions {
  args: "--reload --no-prompt error_015_dynamic_import_permissions.js",
  output: "tests/error_015_dynamic_import_permissions.out",
  check_stderr: true,
  exit_code: 1,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args:
    "--no-prompt --reload --allow-net error_016_dynamic_import_permissions2.js",
  output: "tests/error_016_dynamic_import_permissions2.out",
  check_stderr: true,
  exit_code: 1,
});

itest!(error_stack {
  args: "run --reload error_stack.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_stack.ts.out",
});

itest!(error_syntax {
  args: "run --reload error_syntax.js",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_syntax.js.out",
});

itest!(error_type_definitions {
  args: "run --reload error_type_definitions.ts",
  check_stderr: true,
  exit_code: 1,
  output: "tests/error_type_definitions.ts.out",
});

itest!(exit_error42 {
  exit_code: 42,
  args: "run --reload exit_error42.ts",
  output: "tests/exit_error42.ts.out",
});

itest!(https_import {
  args: "run --reload https_import.ts",
  output: "tests/https_import.ts.out",
});

itest!(if_main {
  args: "run --reload if_main.ts",
  output: "tests/if_main.ts.out",
});

itest!(import_meta {
  args: "run --reload import_meta.ts",
  output: "tests/import_meta.ts.out",
});

itest!(seed_random {
  args: "run --seed=100 seed_random.js",
  output: "tests/seed_random.js.out",
});

itest!(type_definitions {
  args: "run --reload type_definitions.ts",
  output: "tests/type_definitions.ts.out",
});

itest!(types {
  args: "types",
  output: "tests/types.out",
});

itest!(unbuffered_stderr {
  args: "run --reload unbuffered_stderr.ts",
  check_stderr: true,
  output: "tests/unbuffered_stderr.ts.out",
});

itest!(unbuffered_stdout {
  args: "run --reload unbuffered_stdout.ts",
  output: "tests/unbuffered_stdout.ts.out",
});

itest!(v8_flags {
  args: "run --v8-flags=--expose-gc v8_flags.js",
  output: "tests/v8_flags.js.out",
});

itest!(v8_help {
  args: "--v8-options",
  output: "tests/v8_help.out",
});

itest!(version {
  args: "version",
  output: "tests/version.out",
});

itest!(version_long_flag {
  args: "--version",
  output: "tests/version.out",
});

itest!(version_short_flag {
  args: "-v",
  output: "tests/version.out",
});

itest!(wasm {
  args: "run wasm.ts",
  output: "tests/wasm.ts.out",
});

itest!(wasm_async {
  args: "wasm_async.js",
  output: "tests/wasm_async.out",
});

///////////////////////////

#[derive(Debug, Default)]
struct IntegrationTest {
  args: &'static str,
  output: &'static str,
  input: Option<&'static str>,
  exit_code: i32,
  check_stderr: bool,
}

impl IntegrationTest {
  pub fn run(&self) {
    let args = self.args.split_whitespace();
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    let mut target = "debug";

    if let Ok(build_mode) = env::var("DENO_BUILD_MODE") {
      if build_mode == "release" {
        target = "release";
      }
    }

    let bin = root.join(format!("target/{}/deno", target));
    debug!("root path {}", root.display());
    debug!("bin path {}", bin.display());

    let (mut reader, writer) = pipe().unwrap();
    let mut command = Command::new(bin);
    command.args(args);
    command.current_dir(root.join("tests"));
    command.stdin(Stdio::piped());
    command.stderr(Stdio::null());

    if self.check_stderr {
      let writer_clone = writer.try_clone().unwrap();
      command.stderr(writer_clone);
    }

    command.stdout(writer);

    let mut process = command.spawn().expect("failed to execute process");

    if let Some(input) = self.input {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{}", input).unwrap();
    }

    // Very important when using pipes: This parent process is still
    // holding its copies of the write ends, and we have to close them
    // before we read, otherwise the read end will never report EOF. The
    // Command object owns the writers now, and dropping it closes them.
    drop(command);

    let mut actual = String::new();
    reader.read_to_string(&mut actual).unwrap();

    let status = process.wait().expect("failed to finish process");
    let exit_code = status.code().unwrap();

    actual = strip_ansi_codes(&actual).to_string();

    if self.exit_code != exit_code {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      panic!(
        "bad exit code, expected: {:?}, actual: {:?}",
        self.exit_code, exit_code
      );
    }

    let output_path = root.join(self.output);
    debug!("output path {}", output_path.display());
    let expected =
      std::fs::read_to_string(output_path).expect("cannot read output");

    if !wildcard_match(&expected, &actual) {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      println!("EXPECTED\n{}\nEXPECTED", expected);
      panic!("pattern match failed");
    }
  }
}

fn wildcard_match(pattern: &str, s: &str) -> bool {
  pattern_match(pattern, s, "[WILDCARD]")
}

fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
  // Normalize line endings
  let s = s.replace("\r\n", "\n");
  let pattern = pattern.replace("\r\n", "\n");

  if pattern == wildcard {
    return true;
  }

  let parts = pattern.split(wildcard).collect::<Vec<&str>>();
  if parts.len() == 1 {
    return pattern == s;
  }

  if !s.starts_with(parts[0]) {
    return false;
  }

  let mut t = s.split_at(parts[0].len());

  for (i, part) in parts.iter().enumerate() {
    if i == 0 {
      continue;
    }
    dbg!(part, i);
    if i == parts.len() - 1 && (*part == "" || *part == "\n") {
      dbg!("exit 1 true", i);
      return true;
    }
    if let Some(found) = t.1.find(*part) {
      dbg!("found ", found);
      t = t.1.split_at(found + part.len());
    } else {
      dbg!("exit false ", i);
      return false;
    }
  }

  dbg!("end ", t.1.len());
  t.1.is_empty()
}

#[test]
fn test_wildcard_match() {
  let fixtures = vec![
    ("foobarbaz", "foobarbaz", true),
    ("[WILDCARD]", "foobarbaz", true),
    ("foobar", "foobarbaz", false),
    ("foo[WILDCARD]baz", "foobarbaz", true),
    ("foo[WILDCARD]baz", "foobazbar", false),
    ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", true),
    ("foo[WILDCARD]", "foobar", true),
    ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", true),
    // check with different line endings
    ("foo[WILDCARD]\nbaz[WILDCARD]\n", "foobar\nbazqat\n", true),
    (
      "foo[WILDCARD]\nbaz[WILDCARD]\n",
      "foobar\r\nbazqat\r\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\n",
      "foobar\nbazqat\r\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
      "foobar\nbazqat\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
      "foobar\r\nbazqat\r\n",
      true,
    ),
  ];

  // Iterate through the fixture lists, testing each one
  for (pattern, string, expected) in fixtures {
    let actual = wildcard_match(pattern, string);
    dbg!(pattern, string, expected);
    assert_eq!(actual, expected);
  }
}

#[test]
fn test_pattern_match() {
  assert!(pattern_match("foo[BAR]baz", "foobarbaz", "[BAR]"));
  assert!(!pattern_match("foo[BAR]baz", "foobazbar", "[BAR]"));
}
