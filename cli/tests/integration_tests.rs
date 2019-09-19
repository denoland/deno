// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate lazy_static;
extern crate tempfile;
mod util;
use util::*;

#[test]
fn benchmark_test() {
  run_python_script("tools/benchmark_test.py")
}

#[test]
fn deno_dir_test() {
  let g = http_server();
  run_python_script("tools/deno_dir_test.py");
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fetch_test() {
  let g = http_server();
  run_python_script("tools/fetch_test.py");
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn fmt_test() {
  let g = http_server();
  run_python_script("tools/fmt_test.py");
  drop(g);
}

#[test]
fn js_unit_tests() {
  let g = http_server();
  let mut deno = deno_cmd()
    .current_dir(root_path())
    .arg("run")
    .arg("--reload")
    .arg("--allow-run")
    .arg("--allow-env")
    .arg("js/unit_test_runner.ts")
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
  drop(g);
}

// TODO(#2933): Rewrite this test in rust.
#[test]
fn repl_test() {
  run_python_script("tools/repl_test.py")
}

#[test]
fn setup_test() {
  run_python_script("tools/setup_test.py")
}

#[test]
fn target_test() {
  run_python_script("tools/target_test.py")
}

#[test]
fn util_test() {
  run_python_script("tools/util_test.py")
}

macro_rules! itest(
  ($name:ident {$( $key:ident: $value:expr,)*})  => {
    #[test]
    fn $name() {
      (CheckOutputIntegrationTest {
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
  args: "013_dynamic_import.ts --reload --allow-read",
  output: "013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "014_duplicate_import.ts --reload --allow-read",
  output: "014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args: "015_duplicate_parallel_import.js --reload --allow-read",
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

itest!(_030_xeval {
  args: "xeval console.log($.toUpperCase())",
  input: Some("a\nb\n\nc"),
  output: "030_xeval.out",
});

itest!(_031_xeval_replvar {
  args: "xeval -I val console.log(val.toUpperCase());",
  input: Some("a\nb\n\nc"),
  output: "031_xeval_replvar.out",
});

itest!(_032_xeval_delim {
  args: "xeval -d DELIM console.log($.toUpperCase());",
  input: Some("aDELIMbDELIMDELIMc"),
  output: "032_xeval_delim.out",
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

itest!(_035_no_fetch_flag {
  args:
    "--reload --no-fetch http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "035_no_fetch_flag.out",
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

itest!(_039_worker_deno_ns {
  args: "run --reload 039_worker_deno_ns.ts",
  output: "039_worker_deno_ns.ts.out",
});

itest!(_040_worker_blob {
  args: "run --reload 040_worker_blob.ts",
  output: "040_worker_blob.ts.out",
});

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

itest!(async_error {
  exit_code: 1,
  args: "run --reload async_error.ts",
  check_stderr: true,
  output: "async_error.ts.out",
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
  args: "error_014_catch_dynamic_import_error.js --reload --allow-read",
  output: "error_014_catch_dynamic_import_error.js.out",
});

itest!(error_015_dynamic_import_permissions {
  args: "--reload --no-prompt error_015_dynamic_import_permissions.js",
  output: "error_015_dynamic_import_permissions.out",
  check_stderr: true,
  exit_code: 1,
  http_server: true,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args:
    "--no-prompt --reload --allow-net error_016_dynamic_import_permissions2.js",
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
  args: "--v8-options",
  output: "v8_help.out",
});

itest!(version {
  args: "version",
  output: "version.out",
});

itest!(version_long_flag {
  args: "--version",
  output: "version.out",
});

itest!(version_short_flag {
  args: "-v",
  output: "version.out",
});

itest!(wasm {
  args: "run wasm.ts",
  output: "wasm.ts.out",
});

itest!(wasm_async {
  args: "wasm_async.js",
  output: "wasm_async.out",
});
