// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::url;
use std::process::Command;
use std::process::Stdio;
use test_util as util;
use test_util::TempDir;
use util::assert_contains;

itest!(stdout_write_all {
  args: "run --quiet run/stdout_write_all.ts",
  output: "run/stdout_write_all.out",
});

itest!(stdin_read_all {
  args: "run --quiet run/stdin_read_all.ts",
  output: "run/stdin_read_all.out",
  input: Some("01234567890123456789012345678901234567890123456789"),
});

itest!(_001_hello {
  args: "run --reload 001_hello.js",
  output: "001_hello.js.out",
});

itest!(_002_hello {
  args: "run --quiet --reload 002_hello.ts",
  output: "002_hello.ts.out",
});

itest!(_003_relative_import {
  args: "run --quiet --reload 003_relative_import.ts",
  output: "003_relative_import.ts.out",
});

itest!(_004_set_timeout {
  args: "run --quiet --reload 004_set_timeout.ts",
  output: "004_set_timeout.ts.out",
});

itest!(_005_more_imports {
  args: "run --quiet --reload 005_more_imports.ts",
  output: "005_more_imports.ts.out",
});

itest!(_006_url_imports {
  args: "run --quiet --reload 006_url_imports.ts",
  output: "006_url_imports.ts.out",
  http_server: true,
});

itest!(_012_async {
  args: "run --quiet --reload 012_async.ts",
  output: "012_async.ts.out",
});

itest!(_013_dynamic_import {
  args: "run --quiet --reload --allow-read 013_dynamic_import.ts",
  output: "013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "run --quiet --reload --allow-read 014_duplicate_import.ts ",
  output: "014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args: "run --quiet --reload --allow-read 015_duplicate_parallel_import.js",
  output: "015_duplicate_parallel_import.js.out",
});

itest!(_016_double_await {
  args: "run --quiet --allow-read --reload 016_double_await.ts",
  output: "016_double_await.ts.out",
});

itest!(_017_import_redirect {
  args: "run --quiet --reload 017_import_redirect.ts",
  output: "017_import_redirect.ts.out",
});

itest!(_017_import_redirect_nocheck {
  args: "run --quiet --reload --no-check 017_import_redirect.ts",
  output: "017_import_redirect.ts.out",
});

itest!(_017_import_redirect_info {
  args: "info --quiet --reload 017_import_redirect.ts",
  output: "017_import_redirect_info.out",
});

itest!(_018_async_catch {
  args: "run --quiet --reload 018_async_catch.ts",
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
  exit_code: 1,
});

itest!(_021_mjs_modules {
  args: "run --quiet --reload 021_mjs_modules.ts",
  output: "021_mjs_modules.ts.out",
});

itest!(_023_no_ext {
  args: "run --reload --check 023_no_ext",
  output: "023_no_ext.out",
});

// TODO(lucacasonato): remove --unstable when permissions goes stable
itest!(_025_hrtime {
  args: "run --quiet --allow-hrtime --unstable --reload 025_hrtime.ts",
  output: "025_hrtime.ts.out",
});

itest!(_025_reload_js_type_error {
  args: "run --quiet --reload 025_reload_js_type_error.js",
  output: "025_reload_js_type_error.js.out",
});

itest!(_026_redirect_javascript {
  args: "run --quiet --reload 026_redirect_javascript.js",
  output: "026_redirect_javascript.js.out",
  http_server: true,
});

itest!(_027_redirect_typescript {
  args: "run --quiet --reload 027_redirect_typescript.ts",
  output: "027_redirect_typescript.ts.out",
  http_server: true,
});

itest!(_028_args {
  args: "run --quiet --reload 028_args.ts --arg1 val1 --arg2=val2 -- arg3 arg4",
  output: "028_args.ts.out",
});

itest!(_033_import_map {
  args:
    "run --quiet --reload --import-map=import_maps/import_map.json import_maps/test.ts",
  output: "033_import_map.out",
});

itest!(_033_import_map_remote {
  args:
    "run --quiet --reload --import-map=http://127.0.0.1:4545/import_maps/import_map_remote.json --unstable import_maps/test_remote.ts",
  output: "033_import_map_remote.out",
  http_server: true,
});

itest!(_034_onload {
  args: "run --quiet --reload 034_onload/main.ts",
  output: "034_onload.out",
});

itest!(_035_cached_only_flag {
  args: "run --reload --check --cached-only http://127.0.0.1:4545/019_media_types.ts",
  output: "035_cached_only_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_038_checkjs {
  // checking if JS file is run through TS compiler
  args: "run --reload --config checkjs.tsconfig.json --check 038_checkjs.js",
  exit_code: 1,
  output: "038_checkjs.js.out",
});

itest!(_042_dyn_import_evalcontext {
  args: "run --quiet --allow-read --reload 042_dyn_import_evalcontext.ts",
  output: "042_dyn_import_evalcontext.ts.out",
});

itest!(_044_bad_resource {
  args: "run --quiet --reload --allow-read 044_bad_resource.ts",
  output: "044_bad_resource.ts.out",
  exit_code: 1,
});

// TODO(bartlomieju): remove --unstable once Deno.spawn is stabilized
itest!(_045_proxy {
  args: "run -L debug --unstable --allow-net --allow-env --allow-run --allow-read --reload --quiet 045_proxy_test.ts",
  output: "045_proxy_test.ts.out",
  http_server: true,
});

itest!(_046_tsx {
  args: "run --quiet --reload 046_jsx_test.tsx",
  output: "046_jsx_test.tsx.out",
});

itest!(_047_jsx {
  args: "run --quiet --reload 047_jsx_test.jsx",
  output: "047_jsx_test.jsx.out",
});

itest!(_048_media_types_jsx {
  args: "run  --reload 048_media_types_jsx.ts",
  output: "048_media_types_jsx.ts.out",
  http_server: true,
});

itest!(_052_no_remote_flag {
  args:
    "run --reload --check --no-remote http://127.0.0.1:4545/019_media_types.ts",
  output: "052_no_remote_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_056_make_temp_file_write_perm {
  args:
    "run --quiet --allow-read --allow-write=./subdir/ 056_make_temp_file_write_perm.ts",
  output: "056_make_temp_file_write_perm.out",
});

itest!(_058_tasks_microtasks_close {
  args: "run --quiet 058_tasks_microtasks_close.ts",
  output: "058_tasks_microtasks_close.ts.out",
});

itest!(_059_fs_relative_path_perm {
  args: "run 059_fs_relative_path_perm.ts",
  output: "059_fs_relative_path_perm.ts.out",
  exit_code: 1,
});

itest!(_070_location {
  args: "run --location https://foo/bar?baz#bat 070_location.ts",
  output: "070_location.ts.out",
});

itest!(_071_location_unset {
  args: "run 071_location_unset.ts",
  output: "071_location_unset.ts.out",
});

itest!(_072_location_relative_fetch {
  args: "run --location http://127.0.0.1:4545/ --allow-net 072_location_relative_fetch.ts",
  output: "072_location_relative_fetch.ts.out",
  http_server: true,
});

// tests the serialization of webstorage (both localStorage and sessionStorage)
itest!(webstorage_serialization {
  args: "run webstorage/serialization.ts",
  output: "webstorage/serialization.ts.out",
});

// tests the beforeunload event
itest!(beforeunload_event {
  args: "run before_unload.js",
  output: "before_unload.js.out",
});

// tests to ensure that when `--location` is set, all code shares the same
// localStorage cache based on the origin of the location URL.
#[test]
fn webstorage_location_shares_origin() {
  let deno_dir = util::new_deno_dir();

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/a.ts")
    .arg("webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/b.ts")
    .arg("webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 1, hello: \"deno\" }\n");
}

// test to ensure that when a --config file is set, but no --location, that
// storage persists against unique configuration files.
#[test]
fn webstorage_config_file() {
  let deno_dir = util::new_deno_dir();

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--config")
    .arg("webstorage/config_a.jsonc")
    .arg("webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--config")
    .arg("webstorage/config_b.jsonc")
    .arg("webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--config")
    .arg("webstorage/config_a.jsonc")
    .arg("webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 1, hello: \"deno\" }\n");
}

// tests to ensure `--config` does not effect persisted storage when a
// `--location` is provided.
#[test]
fn webstorage_location_precedes_config() {
  let deno_dir = util::new_deno_dir();

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/a.ts")
    .arg("--config")
    .arg("webstorage/config_a.jsonc")
    .arg("webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/b.ts")
    .arg("--config")
    .arg("webstorage/config_b.jsonc")
    .arg("webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 1, hello: \"deno\" }\n");
}

// test to ensure that when there isn't a configuration or location, that the
// main module is used to determine how to persist storage data.
#[test]
fn webstorage_main_module() {
  let deno_dir = util::new_deno_dir();

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 1, hello: \"deno\" }\n");
}

itest!(_075_import_local_query_hash {
  args: "run 075_import_local_query_hash.ts",
  output: "075_import_local_query_hash.ts.out",
});

itest!(_077_fetch_empty {
  args: "run -A 077_fetch_empty.ts",
  output: "077_fetch_empty.ts.out",
  exit_code: 1,
});

itest!(_078_unload_on_exit {
  args: "run 078_unload_on_exit.ts",
  output: "078_unload_on_exit.ts.out",
  exit_code: 1,
});

itest!(_079_location_authentication {
  args: "run --location https://foo:bar@baz/qux 079_location_authentication.ts",
  output: "079_location_authentication.ts.out",
});

itest!(_081_location_relative_fetch_redirect {
    args: "run --location http://127.0.0.1:4546/ --allow-net 081_location_relative_fetch_redirect.ts",
    output: "081_location_relative_fetch_redirect.ts.out",
    http_server: true,
  });

itest!(_082_prepare_stack_trace_throw {
  args: "run 082_prepare_stack_trace_throw.js",
  output: "082_prepare_stack_trace_throw.js.out",
  exit_code: 1,
});

#[test]
fn _083_legacy_external_source_map() {
  let _g = util::http_server();
  let deno_dir = TempDir::new();
  let module_url =
    url::Url::parse("http://localhost:4545/083_legacy_external_source_map.ts")
      .unwrap();
  // Write a faulty old external source map.
  let faulty_map_path = deno_dir.path().join("gen/http/localhost_PORT4545/9576bd5febd0587c5c4d88d57cb3ac8ebf2600c529142abe3baa9a751d20c334.js.map");
  std::fs::create_dir_all(faulty_map_path.parent().unwrap()).unwrap();
  std::fs::write(faulty_map_path, "{\"version\":3,\"file\":\"\",\"sourceRoot\":\"\",\"sources\":[\"http://localhost:4545/083_legacy_external_source_map.ts\"],\"names\":[],\"mappings\":\";AAAA,MAAM,IAAI,KAAK,CAAC,KAAK,CAAC,CAAC\"}").unwrap();
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(module_url.to_string())
    .output()
    .unwrap();
  // Before https://github.com/denoland/deno/issues/6965 was fixed, the faulty
  // old external source map would cause a panic while formatting the error
  // and the exit code would be 101. The external source map should be ignored
  // in favor of the inline one.
  assert_eq!(output.status.code(), Some(1));
  let out = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(out, "");
}

itest!(_085_dynamic_import_async_error {
  args: "run --allow-read 085_dynamic_import_async_error.ts",
  output: "085_dynamic_import_async_error.ts.out",
});

itest!(_086_dynamic_import_already_rejected {
  args: "run --allow-read 086_dynamic_import_already_rejected.ts",
  output: "086_dynamic_import_already_rejected.ts.out",
});

itest!(_087_no_check_imports_not_used_as_values {
    args: "run --config preserve_imports.tsconfig.json --no-check 087_no_check_imports_not_used_as_values.ts",
    output: "087_no_check_imports_not_used_as_values.ts.out",
  });

itest!(_088_dynamic_import_already_evaluating {
  args: "run --allow-read 088_dynamic_import_already_evaluating.ts",
  output: "088_dynamic_import_already_evaluating.ts.out",
});

// TODO(bartlomieju): remove --unstable once Deno.spawn is stabilized
itest!(_089_run_allow_list {
  args: "run --unstable --allow-run=curl 089_run_allow_list.ts",
  output: "089_run_allow_list.ts.out",
});

#[test]
fn _090_run_permissions_request() {
  let args = "run --quiet 090_run_permissions_request.ts";
  use util::PtyData::*;
  util::test_pty2(args, vec![
    Output("⚠️  ️Deno requests run access to \"ls\". Run again with --allow-run to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)]"),
    Input("y\n"),
    Output("⚠️  ️Deno requests run access to \"cat\". Run again with --allow-run to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)]"),
    Input("n\n"),
    Output("granted\r\n"),
    Output("prompt\r\n"),
    Output("denied\r\n"),
  ]);
}

itest!(_091_use_define_for_class_fields {
  args: "run --check 091_use_define_for_class_fields.ts",
  output: "091_use_define_for_class_fields.ts.out",
  exit_code: 1,
});

itest!(_092_import_map_unmapped_bare_specifier {
  args: "run --import-map import_maps/import_map.json 092_import_map_unmapped_bare_specifier.ts",
  output: "092_import_map_unmapped_bare_specifier.ts.out",
  exit_code: 1,
});

itest!(js_import_detect {
  args: "run --quiet --reload js_import_detect.ts",
  output: "js_import_detect.ts.out",
  exit_code: 0,
});

itest!(blob_gc_finalization {
  args: "run blob_gc_finalization.js",
  output: "blob_gc_finalization.js.out",
  exit_code: 0,
});

itest!(fetch_response_finalization {
  args: "run --v8-flags=--expose-gc --allow-net fetch_response_finalization.js",
  output: "fetch_response_finalization.js.out",
  http_server: true,
  exit_code: 0,
});

itest!(import_type {
  args: "run --reload import_type.ts",
  output: "import_type.ts.out",
});

itest!(import_type_no_check {
  args: "run --reload --no-check import_type.ts",
  output: "import_type.ts.out",
});

itest!(private_field_presence {
  args: "run --reload private_field_presence.ts",
  output: "private_field_presence.ts.out",
});

itest!(private_field_presence_no_check {
  args: "run --reload --no-check private_field_presence.ts",
  output: "private_field_presence.ts.out",
});

itest!(lock_write_requires_lock {
  args: "run --lock-write some_file.ts",
  output: "lock_write_requires_lock.out",
  exit_code: 1,
});

// TODO(bartlomieju): remove --unstable once Deno.spawn is stabilized
itest!(lock_write_fetch {
  args:
    "run --quiet --allow-read --allow-write --allow-env --allow-run --unstable lock_write_fetch.ts",
  output: "lock_write_fetch.ts.out",
  http_server: true,
  exit_code: 0,
});

itest!(lock_check_ok {
  args:
    "run --lock=lock_check_ok.json http://127.0.0.1:4545/003_relative_import.ts",
  output: "003_relative_import.ts.out",
  http_server: true,
});

itest!(lock_check_ok2 {
  args: "run --lock=lock_check_ok2.json 019_media_types.ts",
  output: "019_media_types.ts.out",
  http_server: true,
});

itest!(lock_dynamic_imports {
  args: "run --lock=lock_dynamic_imports.json --allow-read --allow-net http://127.0.0.1:4545/013_dynamic_import.ts",
  output: "lock_dynamic_imports.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err {
  args: "run --lock=lock_check_err.json http://127.0.0.1:4545/003_relative_import.ts",
  output: "lock_check_err.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err2 {
  args: "run --lock=lock_check_err2.json 019_media_types.ts",
  output: "lock_check_err2.out",
  exit_code: 10,
  http_server: true,
});

itest!(mts_dmts_mjs {
  args: "run subdir/import.mts",
  output: "mts_dmts_mjs.out",
});

itest!(mts_dmts_mjs_no_check {
  args: "run --no-check subdir/import.mts",
  output: "mts_dmts_mjs.out",
});

itest!(async_error {
  exit_code: 1,
  args: "run --reload async_error.ts",
  output: "async_error.ts.out",
});

itest!(config {
  args: "run --reload --config config.tsconfig.json --check config.ts",
  output: "config.ts.out",
});

itest!(config_types {
  args:
    "run --reload --quiet --config config_types.tsconfig.json config_types.ts",
  output: "config_types.ts.out",
});

itest!(config_types_remote {
    http_server: true,
    args: "run --reload --quiet --config config_types_remote.tsconfig.json config_types.ts",
    output: "config_types.ts.out",
  });

itest!(empty_typescript {
  args: "run --reload --check subdir/empty.ts",
  output_str: Some("Check file:[WILDCARD]/subdir/empty.ts\n"),
});

itest!(error_001 {
  args: "run --reload error_001.ts",
  exit_code: 1,
  output: "error_001.ts.out",
});

itest!(error_002 {
  args: "run --reload error_002.ts",
  exit_code: 1,
  output: "error_002.ts.out",
});

itest!(error_003_typescript {
  args: "run --reload --check error_003_typescript.ts",
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

// Supposing that we've already attempted to run error_003_typescript.ts
// we want to make sure that JS wasn't emitted. Running again without reload flag
// should result in the same output.
// https://github.com/denoland/deno/issues/2436
itest!(error_003_typescript2 {
  args: "run --check error_003_typescript.ts",
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

itest!(error_004_missing_module {
  args: "run --reload error_004_missing_module.ts",
  exit_code: 1,
  output: "error_004_missing_module.ts.out",
});

itest!(error_005_missing_dynamic_import {
  args: "run --reload --allow-read --quiet error_005_missing_dynamic_import.ts",
  exit_code: 1,
  output: "error_005_missing_dynamic_import.ts.out",
});

itest!(error_006_import_ext_failure {
  args: "run --reload error_006_import_ext_failure.ts",
  exit_code: 1,
  output: "error_006_import_ext_failure.ts.out",
});

itest!(error_007_any {
  args: "run --reload error_007_any.ts",
  exit_code: 1,
  output: "error_007_any.ts.out",
});

itest!(error_008_checkjs {
  args: "run --reload error_008_checkjs.js",
  exit_code: 1,
  output: "error_008_checkjs.js.out",
});

itest!(error_009_extensions_error {
  args: "run error_009_extensions_error.js",
  output: "error_009_extensions_error.js.out",
  exit_code: 1,
});

itest!(error_011_bad_module_specifier {
  args: "run --reload error_011_bad_module_specifier.ts",
  exit_code: 1,
  output: "error_011_bad_module_specifier.ts.out",
});

itest!(error_012_bad_dynamic_import_specifier {
  args: "run --reload --check error_012_bad_dynamic_import_specifier.ts",
  exit_code: 1,
  output: "error_012_bad_dynamic_import_specifier.ts.out",
});

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  exit_code: 1,
  output: "error_013_missing_script.out",
});

itest!(error_014_catch_dynamic_import_error {
  args: "run  --reload --allow-read error_014_catch_dynamic_import_error.js",
  output: "error_014_catch_dynamic_import_error.js.out",
});

itest!(error_015_dynamic_import_permissions {
  args: "run --reload --quiet error_015_dynamic_import_permissions.js",
  output: "error_015_dynamic_import_permissions.out",
  exit_code: 1,
  http_server: true,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args: "run --reload --allow-net error_016_dynamic_import_permissions2.js",
  output: "error_016_dynamic_import_permissions2.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_017_hide_long_source_ts {
  args: "run --reload --check error_017_hide_long_source_ts.ts",
  output: "error_017_hide_long_source_ts.ts.out",
  exit_code: 1,
});

itest!(error_018_hide_long_source_js {
  args: "run error_018_hide_long_source_js.js",
  output: "error_018_hide_long_source_js.js.out",
  exit_code: 1,
});

itest!(error_019_stack_function {
  args: "run error_019_stack_function.ts",
  output: "error_019_stack_function.ts.out",
  exit_code: 1,
});

itest!(error_020_stack_constructor {
  args: "run error_020_stack_constructor.ts",
  output: "error_020_stack_constructor.ts.out",
  exit_code: 1,
});

itest!(error_021_stack_method {
  args: "run error_021_stack_method.ts",
  output: "error_021_stack_method.ts.out",
  exit_code: 1,
});

itest!(error_022_stack_custom_error {
  args: "run error_022_stack_custom_error.ts",
  output: "error_022_stack_custom_error.ts.out",
  exit_code: 1,
});

itest!(error_023_stack_async {
  args: "run error_023_stack_async.ts",
  output: "error_023_stack_async.ts.out",
  exit_code: 1,
});

itest!(error_024_stack_promise_all {
  args: "run error_024_stack_promise_all.ts",
  output: "error_024_stack_promise_all.ts.out",
  exit_code: 1,
});

itest!(error_025_tab_indent {
  args: "run error_025_tab_indent",
  output: "error_025_tab_indent.out",
  exit_code: 1,
});

itest!(error_026_remote_import_error {
  args: "run error_026_remote_import_error.ts",
  output: "error_026_remote_import_error.ts.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_for_await {
  args: "run --reload --check error_for_await.ts",
  output: "error_for_await.ts.out",
  exit_code: 1,
});

itest!(error_missing_module_named_import {
  args: "run --reload error_missing_module_named_import.ts",
  output: "error_missing_module_named_import.ts.out",
  exit_code: 1,
});

itest!(error_no_check {
  args: "run --reload --no-check error_no_check.ts",
  output: "error_no_check.ts.out",
  exit_code: 1,
});

itest!(error_syntax {
  args: "run --reload error_syntax.js",
  exit_code: 1,
  output: "error_syntax.js.out",
});

itest!(error_syntax_empty_trailing_line {
  args: "run --reload error_syntax_empty_trailing_line.mjs",
  exit_code: 1,
  output: "error_syntax_empty_trailing_line.mjs.out",
});

itest!(error_type_definitions {
  args: "run --reload --check error_type_definitions.ts",
  exit_code: 1,
  output: "error_type_definitions.ts.out",
});

itest!(error_local_static_import_from_remote_ts {
    args: "run --reload http://localhost:4545/error_local_static_import_from_remote.ts",
    exit_code: 1,
    http_server: true,
    output: "error_local_static_import_from_remote.ts.out",
  });

itest!(error_local_static_import_from_remote_js {
    args: "run --reload http://localhost:4545/error_local_static_import_from_remote.js",
    exit_code: 1,
    http_server: true,
    output: "error_local_static_import_from_remote.js.out",
  });

itest!(exit_error42 {
  exit_code: 42,
  args: "run --quiet --reload exit_error42.ts",
  output: "exit_error42.ts.out",
});

itest!(set_exit_code_0 {
  args: "run --no-check --unstable set_exit_code_0.ts",
  output: "empty.out",
  exit_code: 0,
});

itest!(set_exit_code_1 {
  args: "run --no-check --unstable set_exit_code_1.ts",
  output: "empty.out",
  exit_code: 42,
});

itest!(set_exit_code_2 {
  args: "run --no-check --unstable set_exit_code_2.ts",
  output: "empty.out",
  exit_code: 42,
});

itest!(op_exit_op_set_exit_code_in_worker {
  args: "run --no-check --unstable --allow-read op_exit_op_set_exit_code_in_worker.ts",
  exit_code: 21,
  output: "empty.out",
});

itest!(deno_exit_tampering {
  args: "run --no-check --unstable deno_exit_tampering.ts",
  output: "empty.out",
  exit_code: 42,
});

itest!(heapstats {
  args: "run --quiet --unstable --v8-flags=--expose-gc heapstats.js",
  output: "heapstats.js.out",
});

itest!(finalization_registry {
  args:
    "run --quiet --unstable --v8-flags=--expose-gc finalization_registry.js",
  output: "finalization_registry.js.out",
});

itest!(https_import {
  args: "run --quiet --reload --cert tls/RootCA.pem https_import.ts",
  output: "https_import.ts.out",
  http_server: true,
});

itest!(if_main {
  args: "run --quiet --reload if_main.ts",
  output: "if_main.ts.out",
});

itest!(import_meta {
  args: "run --quiet --reload --import-map=import_meta.importmap.json import_meta.ts",
  output: "import_meta.ts.out",
});

itest!(main_module {
  args: "run --quiet --allow-read --reload main_module.ts",
  output: "main_module.ts.out",
});

itest!(no_check {
  args: "run --quiet --reload --no-check 006_url_imports.ts",
  output: "006_url_imports.ts.out",
  http_server: true,
});

itest!(no_check_decorators {
  args: "run --quiet --reload --no-check no_check_decorators.ts",
  output: "no_check_decorators.ts.out",
});

itest!(check_remote {
  args: "run --quiet --reload --check=all no_check_remote.ts",
  output: "no_check_remote.ts.disabled.out",
  exit_code: 1,
  http_server: true,
});

itest!(no_check_remote {
  args: "run --quiet --reload --no-check=remote no_check_remote.ts",
  output: "no_check_remote.ts.enabled.out",
  http_server: true,
});

itest!(runtime_decorators {
  args: "run --quiet --reload --no-check runtime_decorators.ts",
  output: "runtime_decorators.ts.out",
});

itest!(seed_random {
  args: "run --seed=100 seed_random.js",
  output: "seed_random.js.out",
});

itest!(type_definitions {
  args: "run --reload type_definitions.ts",
  output: "type_definitions.ts.out",
});

itest!(type_definitions_for_export {
  args: "run --reload --check type_definitions_for_export.ts",
  output: "type_definitions_for_export.ts.out",
  exit_code: 1,
});

itest!(type_directives_01 {
  args: "run --reload --check=all -L debug type_directives_01.ts",
  output: "type_directives_01.ts.out",
  http_server: true,
});

itest!(type_directives_02 {
  args: "run --reload --check=all -L debug type_directives_02.ts",
  output: "type_directives_02.ts.out",
});

itest!(type_directives_js_main {
  args: "run --reload -L debug type_directives_js_main.js",
  output: "type_directives_js_main.js.out",
  exit_code: 0,
});

itest!(type_directives_redirect {
  args: "run --reload --check type_directives_redirect.ts",
  output: "type_directives_redirect.ts.out",
  http_server: true,
});

itest!(type_headers_deno_types {
  args: "run --reload --check type_headers_deno_types.ts",
  output: "type_headers_deno_types.ts.out",
  http_server: true,
});

itest!(ts_type_imports {
  args: "run --reload --check ts_type_imports.ts",
  output: "ts_type_imports.ts.out",
  exit_code: 1,
});

itest!(ts_decorators {
  args: "run --reload --check ts_decorators.ts",
  output: "ts_decorators.ts.out",
});

itest!(ts_type_only_import {
  args: "run --reload --check ts_type_only_import.ts",
  output: "ts_type_only_import.ts.out",
});

itest!(swc_syntax_error {
  args: "run --reload --check swc_syntax_error.ts",
  output: "swc_syntax_error.ts.out",
  exit_code: 1,
});

itest!(unbuffered_stderr {
  args: "run --reload unbuffered_stderr.ts",
  output: "unbuffered_stderr.ts.out",
});

itest!(unbuffered_stdout {
  args: "run --quiet --reload unbuffered_stdout.ts",
  output: "unbuffered_stdout.ts.out",
});

itest!(v8_flags_run {
  args: "run --v8-flags=--expose-gc v8_flags.js",
  output: "v8_flags.js.out",
});

itest!(v8_flags_unrecognized {
  args: "repl --v8-flags=--foo,bar,--trace-gc,-baz",
  output: "v8_flags_unrecognized.out",
  exit_code: 1,
});

itest!(v8_help {
  args: "repl --v8-flags=--help",
  output: "v8_help.out",
});

itest!(unsupported_dynamic_import_scheme {
  args: "eval import('xxx:')",
  output: "unsupported_dynamic_import_scheme.out",
  exit_code: 1,
});

itest!(wasm {
  args: "run --quiet wasm.ts",
  output: "wasm.ts.out",
});

itest!(wasm_shared {
  args: "run --quiet wasm_shared.ts",
  output: "wasm_shared.out",
});

itest!(wasm_async {
  args: "run wasm_async.js",
  output: "wasm_async.out",
});

itest!(wasm_unreachable {
  args: "run --allow-read wasm_unreachable.js",
  output: "wasm_unreachable.out",
  exit_code: 1,
});

itest!(wasm_url {
  args: "run --quiet --allow-net=localhost:4545 wasm_url.js",
  output: "wasm_url.out",
  exit_code: 1,
  http_server: true,
});

itest!(weakref {
  args: "run --quiet --reload weakref.ts",
  output: "weakref.ts.out",
});

itest!(top_level_await_order {
  args: "run --allow-read top_level_await_order.js",
  output: "top_level_await_order.out",
});

itest!(top_level_await_loop {
  args: "run --allow-read top_level_await_loop.js",
  output: "top_level_await_loop.out",
});

itest!(top_level_await_circular {
  args: "run --allow-read top_level_await_circular.js",
  output: "top_level_await_circular.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/11238.
itest!(top_level_await_nested {
  args: "run --allow-read top_level_await_nested/main.js",
  output: "top_level_await_nested.out",
});

itest!(top_level_await_unresolved {
  args: "run top_level_await_unresolved.js",
  output: "top_level_await_unresolved.out",
  exit_code: 1,
});

itest!(top_level_await {
  args: "run --allow-read top_level_await.js",
  output: "top_level_await.out",
});

itest!(top_level_await_ts {
  args: "run --quiet --allow-read top_level_await.ts",
  output: "top_level_await.out",
});

itest!(top_level_for_await {
  args: "run --quiet top_level_for_await.js",
  output: "top_level_for_await.out",
});

itest!(top_level_for_await_ts {
  args: "run --quiet top_level_for_await.ts",
  output: "top_level_for_await.out",
});

itest!(unstable_disabled {
  args: "run --reload --check unstable.ts",
  exit_code: 1,
  output: "unstable_disabled.out",
});

itest!(unstable_enabled {
  args: "run --quiet --reload --unstable unstable.ts",
  output: "unstable_enabled.out",
});

itest!(unstable_disabled_js {
  args: "run --reload unstable.js",
  output: "unstable_disabled_js.out",
});

itest!(unstable_enabled_js {
  args: "run --quiet --reload --unstable unstable.ts",
  output: "unstable_enabled_js.out",
});

itest!(unstable_worker {
  args: "run --reload --unstable --quiet --allow-read unstable_worker.ts",
  output: "unstable_worker.ts.out",
});

itest!(_053_import_compression {
  args: "run --quiet --reload --allow-net 053_import_compression/main.ts",
  output: "053_import_compression.out",
  http_server: true,
});

itest!(disallow_http_from_https_js {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/disallow_http_from_https.js",
  output: "disallow_http_from_https_js.out",
  http_server: true,
  exit_code: 1,
});

itest!(disallow_http_from_https_ts {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/disallow_http_from_https.ts",
  output: "disallow_http_from_https_ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(dynamic_import_conditional {
  args: "run --quiet --reload dynamic_import_conditional.js",
  output: "dynamic_import_conditional.js.out",
});

itest!(tsx_imports {
  args: "run --reload --check tsx_imports.ts",
  output: "tsx_imports.ts.out",
});

itest!(fix_dynamic_import_errors {
  args: "run --reload fix_dynamic_import_errors.js",
  output: "fix_dynamic_import_errors.js.out",
});

itest!(fix_emittable_skipped {
  args: "run --reload fix_emittable_skipped.js",
  output: "fix_emittable_skipped.ts.out",
});

itest!(fix_exotic_specifiers {
  args: "run --quiet --reload fix_exotic_specifiers.ts",
  output: "fix_exotic_specifiers.ts.out",
});

itest!(fix_js_import_js {
  args: "run --quiet --reload fix_js_import_js.ts",
  output: "fix_js_import_js.ts.out",
});

itest!(fix_js_imports {
  args: "run --quiet --reload fix_js_imports.ts",
  output: "fix_js_imports.ts.out",
});

itest!(fix_tsc_file_exists {
  args: "run --quiet --reload tsc/test.js",
  output: "fix_tsc_file_exists.out",
});

itest!(fix_worker_dispatchevent {
  args: "run --quiet --reload fix_worker_dispatchevent.ts",
  output: "fix_worker_dispatchevent.ts.out",
});

itest!(es_private_fields {
  args: "run --quiet --reload es_private_fields.js",
  output: "es_private_fields.js.out",
});

itest!(cjs_imports {
  args: "run --quiet --reload cjs_imports.ts",
  output: "cjs_imports.ts.out",
});

itest!(ts_import_from_js {
  args: "run --quiet --reload ts_import_from_js.js",
  output: "ts_import_from_js.js.out",
  http_server: true,
});

itest!(jsx_import_from_ts {
  args: "run --quiet --reload jsx_import_from_ts.ts",
  output: "jsx_import_from_ts.ts.out",
});

itest!(jsx_import_source_pragma {
  args: "run --reload jsx_import_source_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_config {
  args: "run --reload --config jsx/deno-jsx.jsonc jsx_import_source_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_dev_config {
  args:
    "run --reload --config jsx/deno-jsxdev.jsonc jsx_import_source_pragma.tsx",
  output: "jsx_import_source_dev.out",
  http_server: true,
});

itest!(jsx_import_source_no_pragma {
  args:
    "run --reload --config jsx/deno-jsx.jsonc jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_no_pragma_dev {
  args: "run --reload --config jsx/deno-jsxdev.jsonc jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source_dev.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_import_map {
  args: "run --reload --import-map jsx/import-map.json jsx_import_source_pragma_import_map.tsx",
  output: "jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_import_map_dev {
  args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsxdev-import-map.jsonc jsx_import_source_pragma_import_map.tsx",
  output: "jsx_import_source_import_map_dev.out",
  http_server: true,
});

itest!(jsx_import_source_import_map {
  args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsx-import-map.jsonc jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_dev {
  args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsxdev-import-map.jsonc jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source_import_map_dev.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_no_check {
  args: "run --reload --no-check jsx_import_source_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_config_no_check {
  args: "run --reload --config jsx/deno-jsx.jsonc --no-check jsx_import_source_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

// itest!(jsx_import_source_pragma_with_dev_config_no_check {
//   args:
//     "run --reload --config jsx/deno-jsxdev.jsonc --no-check jsx_import_source_pragma.tsx",
//   output: "jsx_import_source_dev.out",
//   http_server: true,
// });

itest!(jsx_import_source_no_pragma_no_check {
  args:
    "run --reload --config jsx/deno-jsx.jsonc --no-check jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source.out",
  http_server: true,
});

// itest!(jsx_import_source_no_pragma_dev_no_check {
//   args: "run --reload --config jsx/deno-jsxdev.jsonc --no-check jsx_import_source_no_pragma.tsx",
//   output: "jsx_import_source_dev.out",
//   http_server: true,
// });

itest!(jsx_import_source_pragma_import_map_no_check {
  args: "run --reload --import-map jsx/import-map.json --no-check jsx_import_source_pragma_import_map.tsx",
  output: "jsx_import_source_import_map.out",
  http_server: true,
});

// itest!(jsx_import_source_pragma_import_map_dev_no_check {
//   args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsxdev-import-map.jsonc --no-check jsx_import_source_pragma_import_map.tsx",
//   output: "jsx_import_source_import_map_dev.out",
//   http_server: true,
// });

itest!(jsx_import_source_import_map_no_check {
  args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsx-import-map.jsonc --no-check jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source_import_map.out",
  http_server: true,
});

// itest!(jsx_import_source_import_map_dev_no_check {
//   args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsxdev-import-map.jsonc --no-check jsx_import_source_no_pragma.tsx",
//   output: "jsx_import_source_import_map_dev.out",
//   http_server: true,
// });

// TODO(#11128): Flaky. Re-enable later.
// itest!(single_compile_with_reload {
//   args: "run --reload --allow-read single_compile_with_reload.ts",
//   output: "single_compile_with_reload.ts.out",
// });

itest!(proto_exploit {
  args: "run proto_exploit.js",
  output: "proto_exploit.js.out",
});

itest!(reference_types {
  args: "run --reload --quiet reference_types.ts",
  output: "reference_types.ts.out",
});

itest!(references_types_remote {
  http_server: true,
  args: "run --reload --quiet reference_types_remote.ts",
  output: "reference_types_remote.ts.out",
});

itest!(import_data_url_error_stack {
  args: "run --quiet --reload import_data_url_error_stack.ts",
  output: "import_data_url_error_stack.ts.out",
  exit_code: 1,
});

itest!(import_data_url_import_relative {
  args: "run --quiet --reload import_data_url_import_relative.ts",
  output: "import_data_url_import_relative.ts.out",
  exit_code: 1,
});

itest!(import_data_url_import_map {
    args: "run --quiet --reload --import-map import_maps/import_map.json import_data_url.ts",
    output: "import_data_url.ts.out",
  });

itest!(import_data_url_imports {
  args: "run --quiet --reload import_data_url_imports.ts",
  output: "import_data_url_imports.ts.out",
  http_server: true,
});

itest!(import_data_url_jsx {
  args: "run --quiet --reload import_data_url_jsx.ts",
  output: "import_data_url_jsx.ts.out",
});

itest!(import_data_url {
  args: "run --quiet --reload import_data_url.ts",
  output: "import_data_url.ts.out",
});

itest!(import_dynamic_data_url {
  args: "run --quiet --reload import_dynamic_data_url.ts",
  output: "import_dynamic_data_url.ts.out",
});

itest!(import_blob_url_error_stack {
  args: "run --quiet --reload import_blob_url_error_stack.ts",
  output: "import_blob_url_error_stack.ts.out",
  exit_code: 1,
});

itest!(import_blob_url_import_relative {
  args: "run --quiet --reload import_blob_url_import_relative.ts",
  output: "import_blob_url_import_relative.ts.out",
  exit_code: 1,
});

itest!(import_blob_url_imports {
  args:
    "run --quiet --reload --allow-net=localhost:4545 import_blob_url_imports.ts",
  output: "import_blob_url_imports.ts.out",
  http_server: true,
});

itest!(import_blob_url_jsx {
  args: "run --quiet --reload import_blob_url_jsx.ts",
  output: "import_blob_url_jsx.ts.out",
});

itest!(import_blob_url {
  args: "run --quiet --reload import_blob_url.ts",
  output: "import_blob_url.ts.out",
});

itest!(import_file_with_colon {
  args: "run --quiet --reload import_file_with_colon.ts",
  output: "import_file_with_colon.ts.out",
  http_server: true,
});

itest!(import_extensionless {
  args: "run --quiet --reload import_extensionless.ts",
  output: "import_extensionless.ts.out",
  http_server: true,
});

itest!(classic_workers_event_loop {
  args:
    "run --enable-testing-features-do-not-use classic_workers_event_loop.js",
  output: "classic_workers_event_loop.js.out",
});

// FIXME(bartlomieju): disabled, because this test is very flaky on CI
// itest!(local_sources_not_cached_in_memory {
//   args: "run --allow-read --allow-write no_mem_cache.js",
//   output: "no_mem_cache.js.out",
// });

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, but is not loaded into the module
// graph (inline_js_source_map_2.ts) (because there are no direct dependencies).
// Source line is not remapped because no inline source contents are included in
// the sourcemap and the file is not present in the dependency graph.
itest!(inline_js_source_map_2 {
  args: "run --quiet inline_js_source_map_2.js",
  output: "inline_js_source_map_2.js.out",
  exit_code: 1,
});

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, but is not loaded into the module
// graph (inline_js_source_map_2.ts) (because there are no direct dependencies).
// Source line remapped using th inline source contents that are included in the
// inline source map.
itest!(inline_js_source_map_2_with_inline_contents {
  args: "run --quiet inline_js_source_map_2_with_inline_contents.js",
  output: "inline_js_source_map_2_with_inline_contents.js.out",
  exit_code: 1,
});

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, and is loaded into the module
// graph because of a direct import statement (inline_js_source_map.ts). The
// source map was generated from an earlier version of this file, where the throw
// was not commented out. The source line is remapped using source contents that
// from the module graph.
itest!(inline_js_source_map_with_contents_from_graph {
  args: "run --quiet inline_js_source_map_with_contents_from_graph.js",
  output: "inline_js_source_map_with_contents_from_graph.js.out",
  exit_code: 1,
  http_server: true,
});

// This test ensures that a descriptive error is shown when we're unable to load
// the import map. Even though this tests only the `run` subcommand, we can be sure
// that the error message is similar for other subcommands as they all use
// `program_state.maybe_import_map` to access the import map underneath.
itest!(error_import_map_unable_to_load {
  args: "run --import-map=import_maps/does_not_exist.json import_maps/test.ts",
  output: "error_import_map_unable_to_load.out",
  exit_code: 1,
});

// Test that setting `self` in the main thread to some other value doesn't break
// the world.
itest!(replace_self {
  args: "run replace_self.js",
  output: "replace_self.js.out",
});

itest!(worker_event_handler_test {
  args: "run --quiet --reload --allow-read worker_event_handler_test.js",
  output: "worker_event_handler_test.js.out",
});

itest!(worker_close_race {
  args: "run --quiet --reload --allow-read worker_close_race.js",
  output: "worker_close_race.js.out",
});

itest!(worker_drop_handle_race {
  args: "run --quiet --reload --allow-read worker_drop_handle_race.js",
  output: "worker_drop_handle_race.js.out",
  exit_code: 1,
});

itest!(worker_drop_handle_race_terminate {
  args: "run --unstable worker_drop_handle_race_terminate.js",
  output: "worker_drop_handle_race_terminate.js.out",
});

itest!(worker_close_nested {
  args: "run --quiet --reload --allow-read worker_close_nested.js",
  output: "worker_close_nested.js.out",
});

itest!(worker_message_before_close {
  args: "run --quiet --reload --allow-read worker_message_before_close.js",
  output: "worker_message_before_close.js.out",
});

itest!(worker_close_in_wasm_reactions {
  args: "run --quiet --reload --allow-read worker_close_in_wasm_reactions.js",
  output: "worker_close_in_wasm_reactions.js.out",
});

itest!(reference_types_error {
  args: "run --config checkjs.tsconfig.json --check reference_types_error.js",
  output: "reference_types_error.js.out",
  exit_code: 1,
});

itest!(reference_types_error_no_check {
  args: "run --no-check reference_types_error.js",
  output_str: Some(""),
});

itest!(jsx_import_source_error {
  args: "run --config jsx/deno-jsx-error.jsonc --check jsx_import_source_no_pragma.tsx",
  output: "jsx_import_source_error.out",
  exit_code: 1,
});

itest!(shebang_tsc {
  args: "run --quiet shebang.ts",
  output: "shebang.ts.out",
});

itest!(shebang_swc {
  args: "run --quiet --no-check shebang.ts",
  output: "shebang.ts.out",
});

itest!(shebang_with_json_imports_tsc {
  args: "run --quiet import_assertions/json_with_shebang.ts",
  output: "import_assertions/json_with_shebang.ts.out",
  exit_code: 1,
});

itest!(shebang_with_json_imports_swc {
  args: "run --quiet --no-check import_assertions/json_with_shebang.ts",
  output: "import_assertions/json_with_shebang.ts.out",
  exit_code: 1,
});

#[test]
fn no_validate_asm() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("no_validate_asm.js")
    .stderr(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(output.stderr.is_empty());
  assert!(output.stdout.is_empty());
}

#[test]
fn exec_path() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--allow-read")
    .arg("exec_path.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  let actual =
    std::fs::canonicalize(&std::path::Path::new(stdout_str)).unwrap();
  let expected = std::fs::canonicalize(util::deno_exe_path()).unwrap();
  assert_eq!(expected, actual);
}

#[cfg(windows)]
// Clippy suggests to remove the `NoStd` prefix from all variants. I disagree.
#[allow(clippy::enum_variant_names)]
enum WinProcConstraints {
  NoStdIn,
  NoStdOut,
  NoStdErr,
}

#[cfg(windows)]
fn run_deno_script_constrained(
  script_path: std::path::PathBuf,
  constraints: WinProcConstraints,
) -> Result<(), i64> {
  let file_path = "DenoWinRunner.ps1";
  let constraints = match constraints {
    WinProcConstraints::NoStdIn => "1",
    WinProcConstraints::NoStdOut => "2",
    WinProcConstraints::NoStdErr => "4",
  };
  let deno_exe_path = util::deno_exe_path()
    .into_os_string()
    .into_string()
    .unwrap();

  let deno_script_path = script_path.into_os_string().into_string().unwrap();

  let args = vec![&deno_exe_path[..], &deno_script_path[..], constraints];
  util::run_powershell_script_file(file_path, args)
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdin() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdIn,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdout() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdOut,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stderr() {
  let output = run_deno_script_constrained(
    util::testdata_path().join("echo.ts"),
    WinProcConstraints::NoStdErr,
  );
  output.unwrap();
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_home_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("HOME")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn should_not_panic_on_undefined_deno_dir_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("DENO_DIR")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_deno_dir_and_home_environment_variables() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("echo.ts")
    .env_remove("DENO_DIR")
    .env_remove("HOME")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn rust_log() {
  // Without RUST_LOG the stderr is empty.
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("001_hello.js")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(output.stderr.is_empty());

  // With RUST_LOG the stderr is not empty.
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("001_hello.js")
    .env("RUST_LOG", "debug")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(!output.stderr.is_empty());
}

#[test]
fn dont_cache_on_check_fail() {
  let deno_dir = util::new_deno_dir();

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check=all")
    .arg("--reload")
    .arg("error_003_typescript.ts")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert!(!output.stderr.is_empty());

  let mut deno_cmd = util::deno_cmd_with_deno_dir(&deno_dir);
  let output = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check=all")
    .arg("error_003_typescript.ts")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  assert!(!output.stderr.is_empty());
}

mod permissions {
  use test_util as util;

  // TODO(bartlomieju): remove --unstable once Deno.spawn is stabilized
  #[test]
  fn with_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg("--unstable")
        .arg(format!("--allow-{0}", permission))
        .arg("permission_test.ts")
        .arg(format!("{0}Required", permission))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  // TODO(bartlomieju): remove --unstable once Deno.spawn is stabilized
  #[test]
  fn without_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!("run --unstable permission_test.ts {0}Required", permission),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_project_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!(
          "--allow-{0}={1}",
          permission,
          util::testdata_path()
            .into_os_string()
            .into_string()
            .unwrap()
        ))
        .arg("complex_permissions_test.ts")
        .arg(permission)
        .arg("complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_outside_test_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!(
          "run --allow-{0}={1} complex_permissions_test.ts {0} {2}",
          permission,
          util::testdata_path()
            .into_os_string()
            .into_string()
            .unwrap(),
          util::root_path()
            .join("Cargo.toml")
            .into_os_string()
            .into_string()
            .unwrap(),
        ),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_test_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!(
          "--allow-{0}={1}",
          permission,
          util::testdata_path()
            .into_os_string()
            .into_string()
            .unwrap()
        ))
        .arg("complex_permissions_test.ts")
        .arg(permission)
        .arg("complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_outside_test_and_js_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    let test_dir = util::testdata_path()
      .into_os_string()
      .into_string()
      .unwrap();
    let js_dir = util::root_path()
      .join("js")
      .into_os_string()
      .into_string()
      .unwrap();
    for permission in &PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!(
          "run --allow-{0}={1},{2} complex_permissions_test.ts {0} {3}",
          permission,
          test_dir,
          js_dir,
          util::root_path()
            .join("Cargo.toml")
            .into_os_string()
            .into_string()
            .unwrap(),
        ),
        None,
        None,
        false,
      );
      assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
    }
  }

  #[test]
  fn rw_inside_test_and_js_dir() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    let test_dir = util::testdata_path()
      .into_os_string()
      .into_string()
      .unwrap();
    let js_dir = util::root_path()
      .join("js")
      .into_os_string()
      .into_string()
      .unwrap();
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{0}={1},{2}", permission, test_dir, js_dir))
        .arg("complex_permissions_test.ts")
        .arg(permission)
        .arg("complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_relative() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{0}=.", permission))
        .arg("complex_permissions_test.ts")
        .arg(permission)
        .arg("complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn rw_no_prefix() {
    const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{0}=tls/../", permission))
        .arg("complex_permissions_test.ts")
        .arg(permission)
        .arg("complex_permissions_test.ts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  #[test]
  fn net_fetch_allow_localhost_4545() {
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost:4545 complex_permissions_test.ts netFetch http://localhost:4545/",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_allow_deno_land() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land complex_permissions_test.ts netFetch http://localhost:4545/",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_localhost_4545_fail() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=localhost:4545 complex_permissions_test.ts netFetch http://localhost:4546/",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_fetch_localhost() {
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost complex_permissions_test.ts netFetch http://localhost:4545/ http://localhost:4546/ http://localhost:4547/",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip_4555() {
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=127.0.0.1:4545 complex_permissions_test.ts netConnect 127.0.0.1:4545",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_deno_land() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land complex_permissions_test.ts netConnect 127.0.0.1:4546",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip_4545_fail() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=127.0.0.1:4545 complex_permissions_test.ts netConnect 127.0.0.1:4546",
        None,
        None,
        true,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_connect_allow_localhost_ip() {
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=127.0.0.1 complex_permissions_test.ts netConnect 127.0.0.1:4545 127.0.0.1:4546 127.0.0.1:4547",
        None,
        None,
        true,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost_4555() {
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost:4558 complex_permissions_test.ts netListen localhost:4558",
        None,
        None,
        false,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_deno_land() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=deno.land complex_permissions_test.ts netListen localhost:4545",
        None,
        None,
        false,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost_4555_fail() {
    let (_, err) = util::run_and_collect_output(
      false,
        "run --allow-net=localhost:4555 complex_permissions_test.ts netListen localhost:4556",
        None,
        None,
        false,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost() {
    // Port 4600 is chosen to not colide with those used by
    // target/debug/test_server
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost complex_permissions_test.ts netListen localhost:4600",
        None,
        None,
        false,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn _061_permissions_request() {
    let args = "run --quiet 061_permissions_request.ts";
    use util::PtyData::*;
    util::test_pty2(args, vec![
      Output("⚠️  ️Deno requests read access to \"foo\". Run again with --allow-read to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)] "),
      Input("y\n"),
      Output("⚠️  ️Deno requests read access to \"bar\". Run again with --allow-read to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)]"),
      Input("n\n"),
      Output("granted\r\n"),
      Output("prompt\r\n"),
      Output("denied\r\n"),
    ]);
  }

  #[test]
  fn _062_permissions_request_global() {
    let args = "run --quiet 062_permissions_request_global.ts";
    use util::PtyData::*;
    util::test_pty2(args, vec![
      Output("⚠️  ️Deno requests read access. Run again with --allow-read to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)] "),
      Input("y\n"),
      Output("PermissionStatus { state: \"granted\", onchange: null }\r\n"),
      Output("PermissionStatus { state: \"granted\", onchange: null }\r\n"),
      Output("PermissionStatus { state: \"granted\", onchange: null }\r\n"),
    ]);
  }

  itest!(_063_permissions_revoke {
    args: "run --allow-read=foo,bar 063_permissions_revoke.ts",
    output: "063_permissions_revoke.ts.out",
  });

  itest!(_064_permissions_revoke_global {
    args: "run --allow-read=foo,bar 064_permissions_revoke_global.ts",
    output: "064_permissions_revoke_global.ts.out",
  });

  #[test]
  fn _066_prompt() {
    let args = "run --quiet --unstable 066_prompt.ts";
    use util::PtyData::*;
    util::test_pty2(
      args,
      vec![
        Output("What is your name? [Jane Doe] "),
        Input("John Doe\n"),
        Output("Your name is John Doe.\r\n"),
        Output("What is your name? [Jane Doe] "),
        Input("\n"),
        Output("Your name is Jane Doe.\r\n"),
        Output("Prompt "),
        Input("foo\n"),
        Output("Your input is foo.\r\n"),
        Output("Question 0 [y/N] "),
        Input("Y\n"),
        Output("Your answer is true\r\n"),
        Output("Question 1 [y/N] "),
        Input("N\n"),
        Output("Your answer is false\r\n"),
        Output("Question 2 [y/N] "),
        Input("yes\n"),
        Output("Your answer is false\r\n"),
        Output("Confirm [y/N] "),
        Input("\n"),
        Output("Your answer is false\r\n"),
        Output("What is Windows EOL? "),
        Input("windows\n"),
        Output("Your answer is \"windows\"\r\n"),
        Output("Hi [Enter] "),
        Input("\n"),
        Output("Alert [Enter] "),
        Input("\n"),
        Output("The end of test\r\n"),
        Output("What is EOF? "),
        Input("\n"),
        Output("Your answer is null\r\n"),
      ],
    );
  }

  itest!(dynamic_import_permissions_remote_remote {
    args: "run --quiet --reload --allow-net=localhost:4545 dynamic_import/permissions_remote_remote.ts",
    output: "dynamic_import/permissions_remote_remote.ts.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(dynamic_import_permissions_data_remote {
    args: "run --quiet --reload --allow-net=localhost:4545 dynamic_import/permissions_data_remote.ts",
    output: "dynamic_import/permissions_data_remote.ts.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(dynamic_import_permissions_blob_remote {
    args: "run --quiet --reload --allow-net=localhost:4545 dynamic_import/permissions_blob_remote.ts",
    output: "dynamic_import/permissions_blob_remote.ts.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(dynamic_import_permissions_data_local {
    args: "run --quiet --reload --allow-net=localhost:4545 dynamic_import/permissions_data_local.ts",
    output: "dynamic_import/permissions_data_local.ts.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(dynamic_import_permissions_blob_local {
    args: "run --quiet --reload --allow-net=localhost:4545 dynamic_import/permissions_blob_local.ts",
    output: "dynamic_import/permissions_blob_local.ts.out",
    http_server: true,
    exit_code: 1,
  });
}

itest!(tls_starttls {
  args: "run --quiet --reload --allow-net --allow-read --unstable --cert tls/RootCA.pem tls_starttls.js",
  output: "tls.out",
});

itest!(tls_connecttls {
  args: "run --quiet --reload --allow-net --allow-read --cert tls/RootCA.pem tls_connecttls.js",
  output: "tls.out",
});

itest!(byte_order_mark {
  args: "run --no-check byte_order_mark.ts",
  output: "byte_order_mark.out",
});

#[test]
fn issue9750() {
  use util::PtyData::*;
  util::test_pty2(
    "run --prompt issue9750.js",
    vec![
      Output("Enter 'yy':\r\n"),
      Input("yy\n"),
      Output("⚠️  ️Deno requests env access. Run again with --allow-env to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)]"),
      Input("n\n"),
      Output("⚠️  ️Deno requests env access to \"SECRET\". Run again with --allow-env to bypass this prompt.\r\n   Allow? [y/n (y = yes allow, n = no deny)]"),
      Input("n\n"),
      Output("error: Uncaught (in promise) PermissionDenied: Requires env access to \"SECRET\", run again with the --allow-env flag\r\n"),
    ],
  );
}

// Regression test for https://github.com/denoland/deno/issues/11451.
itest!(dom_exception_formatting {
  args: "run dom_exception_formatting.ts",
  output: "dom_exception_formatting.ts.out",
  exit_code: 1,
});

itest!(long_data_url_formatting {
  args: "run long_data_url_formatting.ts",
  output: "long_data_url_formatting.ts.out",
  exit_code: 1,
});

itest!(eval_context_throw_dom_exception {
  args: "run eval_context_throw_dom_exception.js",
  output: "eval_context_throw_dom_exception.js.out",
});

/// Regression test for https://github.com/denoland/deno/issues/12740.
#[test]
fn issue12740() {
  let mod_dir = TempDir::new();
  let mod1_path = mod_dir.path().join("mod1.ts");
  let mod2_path = mod_dir.path().join("mod2.ts");
  let mut deno_cmd = util::deno_cmd();
  std::fs::write(&mod1_path, "").unwrap();
  let status = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  std::fs::write(&mod1_path, "export { foo } from \"./mod2.ts\";").unwrap();
  std::fs::write(&mod2_path, "(").unwrap();
  let status = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(!status.success());
}

/// Regression test for https://github.com/denoland/deno/issues/12807.
#[test]
fn issue12807() {
  let mod_dir = TempDir::new();
  let mod1_path = mod_dir.path().join("mod1.ts");
  let mod2_path = mod_dir.path().join("mod2.ts");
  let mut deno_cmd = util::deno_cmd();
  // With a fresh `DENO_DIR`, run a module with a dependency and a type error.
  std::fs::write(&mod1_path, "import './mod2.ts'; Deno.exit('0');").unwrap();
  std::fs::write(&mod2_path, "console.log('Hello, world!');").unwrap();
  let status = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(!status.success());
  // Fix the type error and run again.
  std::fs::write(&mod1_path, "import './mod2.ts'; Deno.exit(0);").unwrap();
  let status = deno_cmd
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check")
    .arg(&mod1_path)
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

itest!(issue_13562 {
  args: "run issue13562.ts",
  output: "issue13562.ts.out",
});

itest!(import_assertions_static_import {
  args: "run --allow-read import_assertions/static_import.ts",
  output: "import_assertions/static_import.out",
});

itest!(import_assertions_static_export {
  args: "run --allow-read import_assertions/static_export.ts",
  output: "import_assertions/static_export.out",
});

itest!(import_assertions_static_error {
  args: "run --allow-read import_assertions/static_error.ts",
  output: "import_assertions/static_error.out",
  exit_code: 1,
});

itest!(import_assertions_dynamic_import {
  args: "run --allow-read import_assertions/dynamic_import.ts",
  output: "import_assertions/dynamic_import.out",
});

itest!(import_assertions_dynamic_error {
  args: "run --allow-read import_assertions/dynamic_error.ts",
  output: "import_assertions/dynamic_error.out",
  exit_code: 1,
});

itest!(import_assertions_type_check {
  args: "run --allow-read --check import_assertions/type_check.ts",
  output: "import_assertions/type_check.out",
  exit_code: 1,
});

itest!(delete_window {
  args: "run delete_window.js",
  output_str: Some("true\n"),
});

itest!(colors_without_global_this {
  args: "run colors_without_globalThis.js",
  output_str: Some("true\n"),
});

itest!(config_auto_discovered_for_local_script {
  args: "run --quiet run/with_config/frontend_work.ts",
  output_str: Some("ok\n"),
});

itest!(no_config_auto_discovery_for_local_script {
  args: "run --quiet --no-config --check run/with_config/frontend_work.ts",
  output: "run/with_config/no_auto_discovery.out",
  exit_code: 1,
});

itest!(config_not_auto_discovered_for_remote_script {
  args: "run --quiet http://127.0.0.1:4545/run/with_config/server_side_work.ts",
  output_str: Some("ok\n"),
  http_server: true,
});

itest!(wasm_streaming_panic_test {
  args: "run wasm_streaming_panic_test.js",
  output: "wasm_streaming_panic_test.js.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/13897.
itest!(fetch_async_error_stack {
  args: "run --quiet -A fetch_async_error_stack.ts",
  output: "fetch_async_error_stack.ts.out",
  exit_code: 1,
});

itest!(unstable_ffi_1 {
  args: "run unstable_ffi_1.js",
  output: "unstable_ffi_1.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_2 {
  args: "run unstable_ffi_2.js",
  output: "unstable_ffi_2.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_3 {
  args: "run unstable_ffi_3.js",
  output: "unstable_ffi_3.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_4 {
  args: "run unstable_ffi_4.js",
  output: "unstable_ffi_4.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_5 {
  args: "run unstable_ffi_5.js",
  output: "unstable_ffi_5.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_6 {
  args: "run unstable_ffi_6.js",
  output: "unstable_ffi_6.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_7 {
  args: "run unstable_ffi_7.js",
  output: "unstable_ffi_7.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_8 {
  args: "run unstable_ffi_8.js",
  output: "unstable_ffi_8.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_9 {
  args: "run unstable_ffi_9.js",
  output: "unstable_ffi_9.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_10 {
  args: "run unstable_ffi_10.js",
  output: "unstable_ffi_10.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_11 {
  args: "run unstable_ffi_11.js",
  output: "unstable_ffi_11.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_12 {
  args: "run unstable_ffi_12.js",
  output: "unstable_ffi_12.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_13 {
  args: "run unstable_ffi_13.js",
  output: "unstable_ffi_13.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_14 {
  args: "run unstable_ffi_14.js",
  output: "unstable_ffi_14.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_15 {
  args: "run unstable_ffi_15.js",
  output: "unstable_ffi_15.js.out",
  exit_code: 70,
});

itest!(future_check2 {
  args: "run --check future_check.ts",
  output: "future_check2.out",
});

itest!(event_listener_error {
  args: "run --quiet event_listener_error.ts",
  output: "event_listener_error.ts.out",
  exit_code: 1,
});

itest!(event_listener_error_handled {
  args: "run --quiet event_listener_error_handled.ts",
  output: "event_listener_error_handled.ts.out",
});

// https://github.com/denoland/deno/pull/14159#issuecomment-1092285446
itest!(event_listener_error_immediate_exit {
  args: "run --quiet event_listener_error_immediate_exit.ts",
  output: "event_listener_error_immediate_exit.ts.out",
  exit_code: 1,
});

// https://github.com/denoland/deno/pull/14159#issuecomment-1092285446
itest!(event_listener_error_immediate_exit_worker {
  args:
    "run --quiet --unstable -A event_listener_error_immediate_exit_worker.ts",
  output: "event_listener_error_immediate_exit_worker.ts.out",
  exit_code: 1,
});

itest!(set_timeout_error {
  args: "run --quiet set_timeout_error.ts",
  output: "set_timeout_error.ts.out",
  exit_code: 1,
});

itest!(set_timeout_error_handled {
  args: "run --quiet set_timeout_error_handled.ts",
  output: "set_timeout_error_handled.ts.out",
});

itest!(aggregate_error {
  args: "run --quiet aggregate_error.ts",
  output: "aggregate_error.out",
  exit_code: 1,
});

itest!(complex_error {
  args: "run --quiet complex_error.ts",
  output: "complex_error.ts.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/12143.
itest!(js_root_with_ts_check {
  args: "run --quiet --check js_root_with_ts_check.js",
  output: "js_root_with_ts_check.js.out",
  exit_code: 1,
});

#[test]
fn check_local_then_remote() {
  let _http_guard = util::http_server();
  let deno_dir = util::new_deno_dir();
  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check")
    .arg("run/remote_type_error/main.ts")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--check=all")
    .arg("run/remote_type_error/main.ts")
    .env("NO_COLOR", "1")
    .stderr(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  assert_contains!(stderr, "Type 'string' is not assignable to type 'number'.");
}

// Regression test for https://github.com/denoland/deno/issues/15163
itest!(check_js_points_to_ts {
  args: "run --quiet --check --config checkjs.tsconfig.json run/check_js_points_to_ts/test.js",
  output: "run/check_js_points_to_ts/test.js.out",
  exit_code: 1,
});

itest!(no_prompt_flag {
  args: "run --quiet --unstable --no-prompt no_prompt.ts",
  output_str: Some(""),
});

#[test]
fn deno_no_prompt_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("no_prompt.ts")
    .env("DENO_NO_PROMPT", "1")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

itest!(report_error {
  args: "run --quiet report_error.ts",
  output: "report_error.ts.out",
  exit_code: 1,
});

itest!(report_error_handled {
  args: "run --quiet report_error_handled.ts",
  output: "report_error_handled.ts.out",
});

itest!(spawn_stdout_inherit {
  args: "run --quiet --unstable -A spawn_stdout_inherit.ts",
  output: "spawn_stdout_inherit.ts.out",
});

itest!(error_name_non_string {
  args: "run --quiet error_name_non_string.js",
  output: "error_name_non_string.js.out",
  exit_code: 1,
});

itest!(custom_inspect_url {
  args: "run custom_inspect_url.js",
  output: "custom_inspect_url.js.out",
});

#[test]
fn running_declaration_files() {
  let temp_dir = TempDir::new();
  let files = vec!["file.d.ts", "file.d.cts", "file.d.mts"];

  for file in files {
    temp_dir.write(file, "");
    let mut deno_cmd = util::deno_cmd_with_deno_dir(&temp_dir);
    let output = deno_cmd
      .current_dir(temp_dir.path())
      .args(["run", file])
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    assert!(output.status.success());
  }
}

itest!(test_and_bench_are_noops_in_run {
  args: "run test_and_bench_in_run.js",
  output_str: Some(""),
});

itest!(followup_dyn_import_resolved {
  args: "run --unstable --allow-read followup_dyn_import_resolves/main.ts",
  output: "followup_dyn_import_resolves/main.ts.out",
});

itest!(unhandled_rejection {
  args: "run --check unhandled_rejection.ts",
  output: "unhandled_rejection.ts.out",
});

itest!(unhandled_rejection_sync_error {
  args: "run --check unhandled_rejection_sync_error.ts",
  output: "unhandled_rejection_sync_error.ts.out",
});

itest!(nested_error {
  args: "run nested_error.ts",
  output: "nested_error.ts.out",
  exit_code: 1,
});
