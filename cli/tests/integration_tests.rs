// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
extern crate tempfile;

#[test]
fn test_pattern_match() {
  assert!(util::pattern_match("foo[BAR]baz", "foobarbaz", "[BAR]"));
  assert!(!util::pattern_match("foo[BAR]baz", "foobazbar", "[BAR]"));
}

#[test]
fn benchmark_test() {
  util::run_python_script("tools/benchmark_test.py")
}

#[test]
fn deno_dir_test() {
  let g = util::http_server();
  util::run_python_script("tools/deno_dir_test.py");
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fetch_test() {
  let g = util::http_server();
  util::run_python_script("tools/fetch_test.py");
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fmt_test() {
  let g = util::http_server();
  util::run_python_script("tools/fmt_test.py");
  drop(g);
}

#[test]
fn js_unit_tests() {
  let g = util::http_server();
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--reload")
    .arg("--allow-run")
    .arg("--allow-env")
    .arg("cli/js/unit_test_runner.ts")
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
  drop(g);
}

#[test]
fn bundle_exports() {
  use tempfile::TempDir;

  // First we have to generate a bundle of some module that has exports.
  let mod1 = util::root_path().join("cli/tests/subdir/mod1.ts");
  assert!(mod1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(mod1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert_eq!(std::str::from_utf8(&output.stdout).unwrap().trim(), "Hello");
  assert_eq!(output.stderr, b"");
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn repl_test() {
  util::run_python_script("tools/repl_test.py")
}

#[test]
fn target_test() {
  util::run_python_script("tools/target_test.py")
}

#[test]
fn util_test() {
  util::run_python_script("tools/util_test.py")
}

macro_rules! itest(
  ($name:ident {$( $key:ident: $value:expr,)*})  => {
    #[test]
    fn $name() {
      (util::CheckOutputIntegrationTest {
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
  output: "001_hello.js.out",
});

itest!(_002_hello {
  args: "run --reload 002_hello.ts",
  output: "002_hello.ts.out",
});

itest!(_003_relative_import {
  args: "run --reload 003_relative_import.ts",
  output: "003_relative_import.ts.out",
});

itest!(_004_set_timeout {
  args: "run --reload 004_set_timeout.ts",
  output: "004_set_timeout.ts.out",
});

itest!(_005_more_imports {
  args: "run --reload 005_more_imports.ts",
  output: "005_more_imports.ts.out",
});

itest!(_006_url_imports {
  args: "run --reload 006_url_imports.ts",
  output: "006_url_imports.ts.out",
  http_server: true,
});

itest!(_012_async {
  args: "run --reload 012_async.ts",
  output: "012_async.ts.out",
});

itest!(_013_dynamic_import {
  args: "run --reload --allow-read 013_dynamic_import.ts",
  output: "013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "run --reload --allow-read 014_duplicate_import.ts ",
  output: "014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args: "run --reload --allow-read 015_duplicate_parallel_import.js",
  output: "015_duplicate_parallel_import.js.out",
});

itest!(_016_double_await {
  args: "run --allow-read --reload 016_double_await.ts",
  output: "016_double_await.ts.out",
});

itest!(_017_import_redirect {
  args: "run --reload 017_import_redirect.ts",
  output: "017_import_redirect.ts.out",
});

itest!(_018_async_catch {
  args: "run --reload 018_async_catch.ts",
  output: "018_async_catch.ts.out",
});

itest!(_019_media_types {
  args: "run --reload 019_media_types.ts",
  output: "019_media_types.ts.out",
  http_server: true,
});

itest!(_020_json_modules {
  args: "run --reload 020_json_modules.ts",
  output: "020_json_modules.ts.out",
});

itest!(_021_mjs_modules {
  args: "run --reload 021_mjs_modules.ts",
  output: "021_mjs_modules.ts.out",
});

itest!(_022_info_flag_script {
  args: "info http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "022_info_flag_script.out",
  http_server: true,
});

itest!(_023_no_ext_with_headers {
  args: "run --reload 023_no_ext_with_headers",
  output: "023_no_ext_with_headers.out",
});

// FIXME(bartlomieju): this test should use remote file
// itest!(_024_import_no_ext_with_headers {
//   args: "run --reload 024_import_no_ext_with_headers.ts",
//   output: "024_import_no_ext_with_headers.ts.out",
// });

itest!(_025_hrtime {
  args: "run --allow-hrtime --reload 025_hrtime.ts",
  output: "025_hrtime.ts.out",
});

itest!(_025_reload_js_type_error {
  args: "run --reload 025_reload_js_type_error.js",
  output: "025_reload_js_type_error.js.out",
});

itest!(_026_redirect_javascript {
  args: "run --reload 026_redirect_javascript.js",
  output: "026_redirect_javascript.js.out",
  http_server: true,
});

itest!(_026_workers {
  args: "run --reload 026_workers.ts",
  output: "026_workers.ts.out",
});

itest!(_027_redirect_typescript {
  args: "run --reload 027_redirect_typescript.ts",
  output: "027_redirect_typescript.ts.out",
  http_server: true,
});

itest!(_028_args {
  args: "run --reload 028_args.ts --arg1 val1 --arg2=val2 -- arg3 arg4",
  output: "028_args.ts.out",
});

itest!(_029_eval {
  args: "eval console.log(\"hello\")",
  output: "029_eval.out",
});

itest!(_033_import_map {
  args:
    "run --reload --importmap=importmaps/import_map.json importmaps/test.ts",
  output: "033_import_map.out",
});

itest!(_034_onload {
  args: "run --reload 034_onload/main.ts",
  output: "034_onload.out",
});

itest!(_035_cached_only_flag {
  args:
    "--reload --cached-only http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "035_cached_only_flag.out",
  exit_code: 1,
  check_stderr: true,
  http_server: true,
});

itest!(_036_import_map_fetch {
  args:
    "fetch --reload --importmap=importmaps/import_map.json importmaps/test.ts",
  output: "036_import_map_fetch.out",
});

itest!(_037_current_thread {
  args: "run --current-thread --reload 034_onload/main.ts",
  output: "034_onload.out",
});

itest!(_038_checkjs {
  // checking if JS file is run through TS compiler
  args: "run --reload --config 038_checkjs.tsconfig.json 038_checkjs.js",
  check_stderr: true,
  exit_code: 1,
  output: "038_checkjs.js.out",
});

/* TODO(bartlomieju):
itest!(_039_worker_deno_ns {
  args: "run --reload 039_worker_deno_ns.ts",
  output: "039_worker_deno_ns.ts.out",
});

itest!(_040_worker_blob {
  args: "run --reload 040_worker_blob.ts",
  output: "040_worker_blob.ts.out",
});
*/

itest!(_041_dyn_import_eval {
  args: "eval import('./subdir/mod4.js').then(console.log)",
  output: "041_dyn_import_eval.out",
});

itest!(_041_info_flag {
  args: "info",
  output: "041_info_flag.out",
});

itest!(_042_dyn_import_evalcontext {
  args: "run --allow-read --reload 042_dyn_import_evalcontext.ts",
  output: "042_dyn_import_evalcontext.ts.out",
});

itest!(_044_bad_resource {
  args: "run --reload --allow-read 044_bad_resource.ts",
  output: "044_bad_resource.ts.out",
  check_stderr: true,
  exit_code: 1,
});

/*
itest!(_045_proxy {
  args: "run --allow-net --allow-env --allow-run --reload 045_proxy_test.ts",
  output: "045_proxy_test.ts.out",
  http_server: true,
});
*/

itest!(_046_tsx {
  args: "run --reload 046_jsx_test.tsx",
  output: "046_jsx_test.tsx.out",
});

itest!(_047_jsx {
  args: "run  --reload 047_jsx_test.jsx",
  output: "047_jsx_test.jsx.out",
});

itest!(_048_media_types_jsx {
  args: "run  --reload 048_media_types_jsx.ts",
  output: "048_media_types_jsx.ts.out",
  http_server: true,
});

itest!(_049_info_flag_script_jsx {
  args: "info http://127.0.0.1:4545/cli/tests/048_media_types_jsx.ts",
  output: "049_info_flag_script_jsx.out",
  http_server: true,
});

itest!(_050_more_jsons {
  args: "run --reload 050_more_jsons.ts",
  output: "050_more_jsons.ts.out",
});

itest!(_051_wasm_import {
  args: "run --reload --allow-net --allow-read 051_wasm_import.ts",
  output: "051_wasm_import.ts.out",
  http_server: true,
});

itest!(_052_no_remote_flag {
  args:
    "--reload --no-remote http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "052_no_remote_flag.out",
  exit_code: 1,
  check_stderr: true,
  http_server: true,
});

itest!(lock_check_ok {
  args: "run --lock=lock_check_ok.json http://127.0.0.1:4545/cli/tests/003_relative_import.ts",
  output: "003_relative_import.ts.out",
  http_server: true,
});

itest!(lock_check_ok2 {
  args: "run 019_media_types.ts --lock=lock_check_ok2.json",
  output: "019_media_types.ts.out",
  http_server: true,
});

itest!(lock_check_err {
  args: "run --lock=lock_check_err.json http://127.0.0.1:4545/cli/tests/003_relative_import.ts",
  output: "lock_check_err.out",
  check_stderr: true,
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err2 {
  args: "run --lock=lock_check_err2.json 019_media_types.ts",
  output: "lock_check_err2.out",
  check_stderr: true,
  exit_code: 10,
  http_server: true,
});

itest!(async_error {
  exit_code: 1,
  args: "run --reload async_error.ts",
  check_stderr: true,
  output: "async_error.ts.out",
});

itest!(bundle {
  args: "bundle subdir/mod1.ts",
  output: "bundle.test.out",
});

itest!(circular1 {
  args: "run --reload circular1.js",
  output: "circular1.js.out",
});

itest!(config {
  args: "run --reload --config config.tsconfig.json config.ts",
  check_stderr: true,
  exit_code: 1,
  output: "config.ts.out",
});

itest!(error_001 {
  args: "run --reload error_001.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_001.ts.out",
});

itest!(error_002 {
  args: "run --reload error_002.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_002.ts.out",
});

itest!(error_003_typescript {
  args: "run --reload error_003_typescript.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

// Supposing that we've already attempted to run error_003_typescript.ts
// we want to make sure that JS wasn't emitted. Running again without reload flag
// should result in the same output.
// https://github.com/denoland/deno/issues/2436
itest!(error_003_typescript2 {
  args: "run error_003_typescript.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

itest!(error_004_missing_module {
  args: "run --reload error_004_missing_module.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_004_missing_module.ts.out",
});

itest!(error_005_missing_dynamic_import {
  args: "run --reload error_005_missing_dynamic_import.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_005_missing_dynamic_import.ts.out",
});

itest!(error_006_import_ext_failure {
  args: "run --reload error_006_import_ext_failure.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_006_import_ext_failure.ts.out",
});

itest!(error_007_any {
  args: "run --reload error_007_any.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_007_any.ts.out",
});

itest!(error_008_checkjs {
  args: "run --reload error_008_checkjs.js",
  check_stderr: true,
  exit_code: 1,
  output: "error_008_checkjs.js.out",
});

itest!(error_011_bad_module_specifier {
  args: "run --reload error_011_bad_module_specifier.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_011_bad_module_specifier.ts.out",
});

itest!(error_012_bad_dynamic_import_specifier {
  args: "run --reload error_012_bad_dynamic_import_specifier.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_012_bad_dynamic_import_specifier.ts.out",
});

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  check_stderr: true,
  exit_code: 1,
  output: "error_013_missing_script.out",
});

itest!(error_014_catch_dynamic_import_error {
  args: "run  --reload --allow-read error_014_catch_dynamic_import_error.js",
  output: "error_014_catch_dynamic_import_error.js.out",
  exit_code: 1,
});

itest!(error_015_dynamic_import_permissions {
  args: "--reload error_015_dynamic_import_permissions.js",
  output: "error_015_dynamic_import_permissions.out",
  check_stderr: true,
  exit_code: 1,
  http_server: true,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args: "--reload --allow-net error_016_dynamic_import_permissions2.js",
  output: "error_016_dynamic_import_permissions2.out",
  check_stderr: true,
  exit_code: 1,
  http_server: true,
});

itest!(error_stack {
  args: "run --reload error_stack.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_stack.ts.out",
});

itest!(error_syntax {
  args: "run --reload error_syntax.js",
  check_stderr: true,
  exit_code: 1,
  output: "error_syntax.js.out",
});

itest!(error_type_definitions {
  args: "run --reload error_type_definitions.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_type_definitions.ts.out",
});

/* TODO(bartlomieju)
itest!(error_worker_dynamic {
  args: "run --reload error_worker_dynamic.ts",
  check_stderr: true,
  exit_code: 1,
  output: "error_worker_dynamic.ts.out",
});
*/

itest!(exit_error42 {
  exit_code: 42,
  args: "run --reload exit_error42.ts",
  output: "exit_error42.ts.out",
});

itest!(https_import {
  args: "run --reload https_import.ts",
  output: "https_import.ts.out",
});

itest!(if_main {
  args: "run --reload if_main.ts",
  output: "if_main.ts.out",
});

itest!(import_meta {
  args: "run --reload import_meta.ts",
  output: "import_meta.ts.out",
});

itest!(seed_random {
  args: "run --seed=100 seed_random.js",
  output: "seed_random.js.out",
});

itest!(type_definitions {
  args: "run --reload type_definitions.ts",
  output: "type_definitions.ts.out",
});

itest!(types {
  args: "types",
  output: "types.out",
});

itest!(unbuffered_stderr {
  args: "run --reload unbuffered_stderr.ts",
  check_stderr: true,
  output: "unbuffered_stderr.ts.out",
});

itest!(unbuffered_stdout {
  args: "run --reload unbuffered_stdout.ts",
  output: "unbuffered_stdout.ts.out",
});

itest!(v8_flags {
  args: "run --v8-flags=--expose-gc v8_flags.js",
  output: "v8_flags.js.out",
});

itest!(v8_help {
  args: "run --v8-flags=--help",
  output: "v8_help.out",
});

itest!(wasm {
  args: "run wasm.ts",
  output: "wasm.ts.out",
});

itest!(wasm_async {
  args: "wasm_async.js",
  output: "wasm_async.out",
});

itest!(top_level_await {
  args: "--allow-read top_level_await.js",
  output: "top_level_await.out",
});

itest!(top_level_await_ts {
  args: "--allow-read top_level_await.ts",
  output: "top_level_await.out",
});

itest!(top_level_for_await {
  args: "top_level_for_await.js",
  output: "top_level_for_await.out",
});

itest!(top_level_for_await_ts {
  args: "top_level_for_await.ts",
  output: "top_level_for_await.out",
});

itest!(_053_import_compression {
  args: "run --reload --allow-net 053_import_compression/main.ts",
  output: "053_import_compression.out",
  http_server: true,
});

mod util {
  use deno::colors::strip_ansi_codes;
  pub use deno::test_util::*;
  use os_pipe::pipe;
  use std::io::Read;
  use std::io::Write;
  use std::process::Command;
  use std::process::Stdio;
  use tempfile::TempDir;

  lazy_static! {
    static ref DENO_DIR: TempDir = { TempDir::new().expect("tempdir fail") };
  }

  pub fn deno_cmd() -> Command {
    let mut c = Command::new(deno_exe_path());
    c.env("DENO_DIR", DENO_DIR.path());
    c
  }

  pub fn run_python_script(script: &str) {
    let output = Command::new("python")
      .env("DENO_DIR", DENO_DIR.path())
      .current_dir(root_path())
      .arg(script)
      .arg(format!("--executable={}", deno_exe_path().display()))
      .env("DENO_BUILD_PATH", target_dir())
      .output()
      .expect("failed to spawn script");
    if !output.status.success() {
      let stdout = String::from_utf8(output.stdout).unwrap();
      let stderr = String::from_utf8(output.stderr).unwrap();
      panic!(
        "{} executed with failing error code\n{}{}",
        script, stdout, stderr
      );
    }
  }

  #[derive(Debug, Default)]
  pub struct CheckOutputIntegrationTest {
    pub args: &'static str,
    pub output: &'static str,
    pub input: Option<&'static str>,
    pub exit_code: i32,
    pub check_stderr: bool,
    pub http_server: bool,
  }

  impl CheckOutputIntegrationTest {
    pub fn run(&self) {
      let args = self.args.split_whitespace();
      let root = root_path();
      let deno_exe = deno_exe_path();
      println!("root path {}", root.display());
      println!("deno_exe path {}", deno_exe.display());

      let http_server_guard = if self.http_server {
        Some(http_server())
      } else {
        None
      };

      let (mut reader, writer) = pipe().unwrap();
      let tests_dir = root.join("cli").join("tests");
      let mut command = deno_cmd();
      command.args(args);
      command.current_dir(&tests_dir);
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

      drop(http_server_guard);

      actual = strip_ansi_codes(&actual).to_string();

      if self.exit_code != exit_code {
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!(
          "bad exit code, expected: {:?}, actual: {:?}",
          self.exit_code, exit_code
        );
      }

      let output_path = tests_dir.join(self.output);
      println!("output path {}", output_path.display());
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

  pub fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
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
}
