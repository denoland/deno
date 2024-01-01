// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use deno_core::serde_json::json;
use deno_core::url;
use deno_runtime::deno_fetch::reqwest;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use test_util as util;
use test_util::TempDir;
use trust_dns_client::serialize::txt::Lexer;
use trust_dns_client::serialize::txt::Parser;
use util::assert_contains;
use util::assert_not_contains;
use util::env_vars_for_npm_tests;
use util::PathRef;
use util::TestContext;
use util::TestContextBuilder;

itest!(stdout_write_all {
  args: "run --quiet run/stdout_write_all.ts",
  output: "run/stdout_write_all.out",
});

itest!(stdin_read_all {
  args: "run --quiet run/stdin_read_all.ts",
  output: "run/stdin_read_all.out",
  input: Some("01234567890123456789012345678901234567890123456789"),
});

itest!(stdout_write_sync_async {
  args: "run --quiet run/stdout_write_sync_async.ts",
  output: "run/stdout_write_sync_async.out",
});

itest!(_001_hello {
  args: "run --reload run/001_hello.js",
  output: "run/001_hello.js.out",
});

itest!(_002_hello {
  args: "run --quiet --reload run/002_hello.ts",
  output: "run/002_hello.ts.out",
});

itest!(_003_relative_import {
  args: "run --quiet --reload run/003_relative_import.ts",
  output: "run/003_relative_import.ts.out",
});

itest!(_004_set_timeout {
  args: "run --quiet --reload run/004_set_timeout.ts",
  output: "run/004_set_timeout.ts.out",
});

itest!(_005_more_imports {
  args: "run --quiet --reload run/005_more_imports.ts",
  output: "run/005_more_imports.ts.out",
});

itest!(_006_url_imports {
  args: "run --quiet --reload run/006_url_imports.ts",
  output: "run/006_url_imports.ts.out",
  http_server: true,
});

itest!(_012_async {
  args: "run --quiet --reload run/012_async.ts",
  output: "run/012_async.ts.out",
});

itest!(_013_dynamic_import {
  args: "run --quiet --reload --allow-read run/013_dynamic_import.ts",
  output: "run/013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "run --quiet --reload --allow-read run/014_duplicate_import.ts ",
  output: "run/014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args:
    "run --quiet --reload --allow-read run/015_duplicate_parallel_import.js",
  output: "run/015_duplicate_parallel_import.js.out",
});

itest!(_016_double_await {
  args: "run --quiet --allow-read --reload run/016_double_await.ts",
  output: "run/016_double_await.ts.out",
});

itest!(_017_import_redirect {
  args: "run --quiet --reload run/017_import_redirect.ts",
  output: "run/017_import_redirect.ts.out",
});

itest!(_017_import_redirect_check {
  args: "run --quiet --reload --check run/017_import_redirect.ts",
  output: "run/017_import_redirect.ts.out",
});

itest!(_017_import_redirect_vendor_dir {
  args:
    "run --quiet --reload --vendor --check $TESTDATA/run/017_import_redirect.ts",
  output: "run/017_import_redirect.ts.out",
  temp_cwd: true,
});

itest!(_017_import_redirect_info {
  args: "info --quiet --reload run/017_import_redirect.ts",
  output: "run/017_import_redirect_info.out",
});

itest!(_018_async_catch {
  args: "run --quiet --reload run/018_async_catch.ts",
  output: "run/018_async_catch.ts.out",
});

itest!(_019_media_types {
  args: "run --reload run/019_media_types.ts",
  output: "run/019_media_types.ts.out",
  http_server: true,
});

itest!(_020_json_modules {
  args: "run --reload run/020_json_modules.ts",
  output: "run/020_json_modules.ts.out",
  exit_code: 1,
});

itest!(_021_mjs_modules {
  args: "run --quiet --reload run/021_mjs_modules.ts",
  output: "run/021_mjs_modules.ts.out",
});

itest!(_023_no_ext {
  args: "run --reload --check run/023_no_ext",
  output: "run/023_no_ext.out",
});

// TODO(lucacasonato): remove --unstable when permissions goes stable
itest!(_025_hrtime {
  args: "run --quiet --allow-hrtime --unstable --reload run/025_hrtime.ts",
  output: "run/025_hrtime.ts.out",
});

itest!(_025_reload_js_type_error {
  args: "run --quiet --reload run/025_reload_js_type_error.js",
  output: "run/025_reload_js_type_error.js.out",
});

itest!(_026_redirect_javascript {
  args: "run --quiet --reload run/026_redirect_javascript.js",
  output: "run/026_redirect_javascript.js.out",
  http_server: true,
});

itest!(_027_redirect_typescript {
  args: "run --quiet --reload run/027_redirect_typescript.ts",
  output: "run/027_redirect_typescript.ts.out",
  http_server: true,
});

itest!(_027_redirect_typescript_vendor_dir {
  args:
    "run --quiet --reload --vendor $TESTDATA/run/027_redirect_typescript.ts",
  output: "run/027_redirect_typescript.ts.out",
  http_server: true,
  temp_cwd: true,
});

itest!(_028_args {
  args:
    "run --quiet --reload run/028_args.ts --arg1 val1 --arg2=val2 -- arg3 arg4",
  output: "run/028_args.ts.out",
});

itest!(_033_import_map {
  args:
    "run --quiet --reload --import-map=import_maps/import_map.json import_maps/test.ts",
  output: "run/033_import_map.out",
});

itest!(_033_import_map_in_config_file {
  args: "run --reload --config=import_maps/config.json import_maps/test.ts",
  output: "run/033_import_map_in_config_file.out",
});

itest!(_033_import_map_in_flag_has_precedence {
  args: "run --quiet --reload --import-map=import_maps/import_map_invalid.json --config=import_maps/config.json import_maps/test.ts",
  output: "run/033_import_map_in_flag_has_precedence.out",
  exit_code: 1,
});

itest!(_033_import_map_remote {
  args:
    "run --quiet --reload --import-map=http://127.0.0.1:4545/import_maps/import_map_remote.json --unstable import_maps/test_remote.ts",
  output: "run/033_import_map_remote.out",
  http_server: true,
});

itest!(_033_import_map_vendor_dir_remote {
  args:
    "run --quiet --reload --import-map=http://127.0.0.1:4545/import_maps/import_map_remote.json --vendor --unstable $TESTDATA/import_maps/test_remote.ts",
  output: "run/033_import_map_remote.out",
  http_server: true,
  temp_cwd: true,
});

itest!(_033_import_map_data_uri {
  args:
    "run --quiet --reload --import-map=data:application/json;charset=utf-8;base64,ewogICJpbXBvcnRzIjogewogICAgInRlc3Rfc2VydmVyLyI6ICJodHRwOi8vbG9jYWxob3N0OjQ1NDUvIgogIH0KfQ== run/import_maps/test_data.ts",
  output: "run/import_maps/test_data.ts.out",
  http_server: true,
});

itest!(onload {
  args: "run --quiet --reload run/onload/main.ts",
  output: "run/onload/main.out",
});

itest!(_035_cached_only_flag {
  args: "run --reload --check --cached-only http://127.0.0.1:4545/run/019_media_types.ts",
  output: "run/035_cached_only_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_038_checkjs {
  // checking if JS file is run through TS compiler
  args:
    "run --reload --config run/checkjs.tsconfig.json --check run/038_checkjs.js",
  exit_code: 1,
  output: "run/038_checkjs.js.out",
});

itest!(_042_dyn_import_evalcontext {
  args: "run --quiet --allow-read --reload run/042_dyn_import_evalcontext.ts",
  output: "run/042_dyn_import_evalcontext.ts.out",
});

itest!(_044_bad_resource {
  args: "run --quiet --reload --allow-read run/044_bad_resource.ts",
  output: "run/044_bad_resource.ts.out",
  exit_code: 1,
});

// TODO(bartlomieju): remove --unstable once Deno.Command is stabilized
itest!(_045_proxy {
  args: "run -L debug --unstable --allow-net --allow-env --allow-run --allow-read --reload --quiet run/045_proxy_test.ts",
  output: "run/045_proxy_test.ts.out",
  http_server: true,
});

itest!(_046_tsx {
  args: "run --quiet --reload run/046_jsx_test.tsx",
  output: "run/046_jsx_test.tsx.out",
});

itest!(_047_jsx {
  args: "run --quiet --reload run/047_jsx_test.jsx",
  output: "run/047_jsx_test.jsx.out",
});

itest!(_048_media_types_jsx {
  args: "run  --reload run/048_media_types_jsx.ts",
  output: "run/048_media_types_jsx.ts.out",
  http_server: true,
});

itest!(_052_no_remote_flag {
  args:
    "run --reload --check --no-remote http://127.0.0.1:4545/run/019_media_types.ts",
  output: "run/052_no_remote_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_056_make_temp_file_write_perm {
  args:
    "run --quiet --allow-read --allow-write=./subdir/ run/056_make_temp_file_write_perm.ts",
  output: "run/056_make_temp_file_write_perm.out",
});

itest!(_058_tasks_microtasks_close {
  args: "run --quiet run/058_tasks_microtasks_close.ts",
  output: "run/058_tasks_microtasks_close.ts.out",
});

itest!(_059_fs_relative_path_perm {
  args: "run run/059_fs_relative_path_perm.ts",
  output: "run/059_fs_relative_path_perm.ts.out",
  exit_code: 1,
});

itest!(_070_location {
  args: "run --location https://foo/bar?baz#bat run/070_location.ts",
  output: "run/070_location.ts.out",
});

itest!(_071_location_unset {
  args: "run run/071_location_unset.ts",
  output: "run/071_location_unset.ts.out",
});

itest!(_072_location_relative_fetch {
  args: "run --location http://127.0.0.1:4545/ --allow-net run/072_location_relative_fetch.ts",
  output: "run/072_location_relative_fetch.ts.out",
  http_server: true,
});

// tests the beforeunload event
itest!(beforeunload_event {
  args: "run run/before_unload.js",
  output: "run/before_unload.js.out",
});

// tests the serialization of webstorage (both localStorage and sessionStorage)
itest!(webstorage_serialization {
  args: "run run/webstorage/serialization.ts",
  output: "run/webstorage/serialization.ts.out",
});

// tests to ensure that when `--location` is set, all code shares the same
// localStorage cache based on the origin of the location URL.
#[test]
fn webstorage_location_shares_origin() {
  let deno_dir = util::new_deno_dir();

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/a.ts")
    .arg("run/webstorage/fixture.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { length: 0 }\n");

  let output = util::deno_cmd_with_deno_dir(&deno_dir)
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--location")
    .arg("https://example.com/b.ts")
    .arg("run/webstorage/logger.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, b"Storage { hello: \"deno\", length: 1 }\n");
}

// test to ensure that when a --config file is set, but no --location, that
// storage persists against unique configuration files.
#[test]
fn webstorage_config_file() {
  let context = TestContext::default();

  context
    .new_command()
    .args(
      "run --config run/webstorage/config_a.jsonc run/webstorage/fixture.ts",
    )
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run --config run/webstorage/config_b.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run --config run/webstorage/config_a.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

// tests to ensure `--config` does not effect persisted storage when a
// `--location` is provided.
#[test]
fn webstorage_location_precedes_config() {
  let context = TestContext::default();

  context.new_command()
    .args("run --location https://example.com/a.ts --config run/webstorage/config_a.jsonc run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context.new_command()
    .args("run --location https://example.com/b.ts --config run/webstorage/config_b.jsonc run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

// test to ensure that when there isn't a configuration or location, that the
// main module is used to determine how to persist storage data.
#[test]
fn webstorage_main_module() {
  let context = TestContext::default();

  context
    .new_command()
    .args("run run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run run/webstorage/logger.ts")
    .run()
    .assert_matches_text("Storage { length: 0 }\n");

  context
    .new_command()
    .args("run run/webstorage/fixture.ts")
    .run()
    .assert_matches_text("Storage { hello: \"deno\", length: 1 }\n");
}

itest!(_075_import_local_query_hash {
  args: "run run/075_import_local_query_hash.ts",
  output: "run/075_import_local_query_hash.ts.out",
});

itest!(_077_fetch_empty {
  args: "run -A run/077_fetch_empty.ts",
  output: "run/077_fetch_empty.ts.out",
  exit_code: 1,
});

itest!(_078_unload_on_exit {
  args: "run run/078_unload_on_exit.ts",
  output: "run/078_unload_on_exit.ts.out",
  exit_code: 1,
});

itest!(_079_location_authentication {
  args:
    "run --location https://foo:bar@baz/qux run/079_location_authentication.ts",
  output: "run/079_location_authentication.ts.out",
});

itest!(_081_location_relative_fetch_redirect {
    args: "run --location http://127.0.0.1:4546/ --allow-net run/081_location_relative_fetch_redirect.ts",
    output: "run/081_location_relative_fetch_redirect.ts.out",
    http_server: true,
  });

itest!(_082_prepare_stack_trace_throw {
  args: "run run/082_prepare_stack_trace_throw.js",
  output: "run/082_prepare_stack_trace_throw.js.out",
  exit_code: 1,
});

#[test]
fn _083_legacy_external_source_map() {
  let _g = util::http_server();
  let deno_dir = TempDir::new();
  let module_url = url::Url::parse(
    "http://localhost:4545/run/083_legacy_external_source_map.ts",
  )
  .unwrap();
  // Write a faulty old external source map.
  let faulty_map_path = deno_dir.path().join("gen/http/localhost_PORT4545/9576bd5febd0587c5c4d88d57cb3ac8ebf2600c529142abe3baa9a751d20c334.js.map");
  faulty_map_path.parent().create_dir_all();
  faulty_map_path.write(r#"{\"version\":3,\"file\":\"\",\"sourceRoot\":\"\",\"sources\":[\"http://localhost:4545/083_legacy_external_source_map.ts\"],\"names\":[],\"mappings\":\";AAAA,MAAM,IAAI,KAAK,CAAC,KAAK,CAAC,CAAC\"}"#);
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

itest!(dynamic_import_async_error {
  args: "run --allow-read run/dynamic_import_async_error/main.ts",
  output: "run/dynamic_import_async_error/main.out",
});

itest!(dynamic_import_already_rejected {
  args: "run --allow-read run/dynamic_import_already_rejected/main.ts",
  output: "run/dynamic_import_already_rejected/main.out",
});

itest!(dynamic_import_concurrent_non_statically_analyzable {
  args: "run --allow-read --allow-net --quiet run/dynamic_import_concurrent_non_statically_analyzable/main.ts",
  output: "run/dynamic_import_concurrent_non_statically_analyzable/main.out",
  http_server: true,
});

itest!(no_check_imports_not_used_as_values {
    args: "run --config run/no_check_imports_not_used_as_values/preserve_imports.tsconfig.json --no-check run/no_check_imports_not_used_as_values/main.ts",
    output: "run/no_check_imports_not_used_as_values/main.out",
  });

itest!(_088_dynamic_import_already_evaluating {
  args: "run --allow-read run/088_dynamic_import_already_evaluating.ts",
  output: "run/088_dynamic_import_already_evaluating.ts.out",
});

// TODO(bartlomieju): remove --unstable once Deno.Command is stabilized
itest!(_089_run_allow_list {
  args: "run --unstable --allow-run=curl run/089_run_allow_list.ts",
  output: "run/089_run_allow_list.ts.out",
});

#[test]
fn _090_run_permissions_request() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/090_run_permissions_request.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"ls\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("y");
      console.expect("Granted run access to \"ls\".");
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"cat\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("n");
      console.expect("Denied run access to \"cat\".");
      console.expect("granted");
      console.expect("denied");
    });
}

#[test]
fn _090_run_permissions_request_sync() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/090_run_permissions_request_sync.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"ls\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("y");
      console.expect("Granted run access to \"ls\".");
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"cat\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("n");
      console.expect("Denied run access to \"cat\".");
      console.expect("granted");
      console.expect("denied");
    });
}

#[test]
fn permissions_prompt_allow_all() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all.ts"])
    .with_pty(|mut console| {
      // "run" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all run access.");
      // "read" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests read access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-read to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all read access.");
      // "write" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests write access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-write to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all write permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all write access.");
      // "net" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests net access to \"foo\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-net to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all net permissions)",
      ));
      console.write_line_raw("A\n");
      console.expect("✅ Granted all net access.");
      // "env" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests env access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-env to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.write_line_raw("A\n");
      console.expect("✅ Granted all env access.");
      // "sys" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests sys access to \"loadavg\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-sys to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)",
      ));
      console.write_line_raw("A\n");
      console.expect("✅ Granted all sys access.");
      // "ffi" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests ffi access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-ffi to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all ffi permissions)",
      ));
      console.write_line_raw("A\n");
      console.expect("✅ Granted all ffi access.")
    },
  );
}

#[test]
fn permissions_prompt_allow_all_2() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all_2.ts"])
    .with_pty(|mut console| {
      // "env" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests env access to \"FOO\".\r\n",
        "├ Run again with --allow-env to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all env access.");

      // "sys" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests sys access to \"loadavg\".\r\n",
        "├ Requested by `Deno.loadavg()` API.\r\n",
        "├ Run again with --allow-sys to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all sys permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all sys access.");

      // "read" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests read access to <CWD>.\r\n",
        "├ Requested by `Deno.cwd()` API.\r\n",
        "├ Run again with --allow-read to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.write_line_raw("A");
      console.expect("✅ Granted all read access.");
    });
}

#[test]
fn permissions_prompt_allow_all_lowercase_a() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_prompt_allow_all.ts"])
    .with_pty(|mut console| {
      // "run" permissions
      console.expect(concat!(
        "┌ ⚠️  Deno requests run access to \"FOO\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-run to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all run permissions)",
      ));
      console.write_line_raw("a");
      console.expect("Unrecognized option.");
    });
}

itest!(deny_all_permission_args {
  args: "run --deny-env --deny-read --deny-write --deny-ffi --deny-run --deny-sys --deny-net --deny-hrtime run/deny_all_permission_args.js",
  output: "run/deny_all_permission_args.out",
});

itest!(deny_some_permission_args {
  args: "run --allow-env --deny-env=FOO --allow-read --deny-read=/foo --allow-write --deny-write=/foo --allow-ffi --deny-ffi=/foo --allow-run --deny-run=foo --allow-sys --deny-sys=hostname --allow-net --deny-net=127.0.0.1 --allow-hrtime --deny-hrtime run/deny_some_permission_args.js",
  output: "run/deny_some_permission_args.out",
});

#[test]
fn permissions_cache() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--quiet", "run/permissions_cache.ts"])
    .with_pty(|mut console| {
      console.expect(concat!(
        "prompt\r\n",
        "┌ ⚠️  Deno requests read access to \"foo\".\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-read to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
      ));
      console.write_line_raw("y");
      console.expect("✅ Granted read access to \"foo\".");
      console.expect("granted");
      console.expect("prompt");
    });
}

itest!(env_file {
  args: "run --env=env --allow-env run/env_file.ts",
  output: "run/env_file.out",
});

itest!(env_file_missing {
  args: "run --env=missing --allow-env run/env_file.ts",
  output_str: Some(
    "error: Unable to load 'missing' environment variable file\n"
  ),
  exit_code: 1,
});

itest!(_091_use_define_for_class_fields {
  args: "run --check run/091_use_define_for_class_fields.ts",
  output: "run/091_use_define_for_class_fields.ts.out",
  exit_code: 1,
});

itest!(_092_import_map_unmapped_bare_specifier {
  args: "run --import-map import_maps/import_map.json run/092_import_map_unmapped_bare_specifier.ts",
  output: "run/092_import_map_unmapped_bare_specifier.ts.out",
  exit_code: 1,
});

itest!(js_import_detect {
  args: "run --quiet --reload run/js_import_detect.ts",
  output: "run/js_import_detect.ts.out",
  exit_code: 0,
});

itest!(blob_gc_finalization {
  args: "run run/blob_gc_finalization.js",
  output: "run/blob_gc_finalization.js.out",
  exit_code: 0,
});

itest!(fetch_response_finalization {
  args:
    "run --v8-flags=--expose-gc --allow-net run/fetch_response_finalization.js",
  output: "run/fetch_response_finalization.js.out",
  http_server: true,
  exit_code: 0,
});

itest!(import_type {
  args: "run --reload run/import_type.ts",
  output: "run/import_type.ts.out",
});

itest!(import_type_no_check {
  args: "run --reload --no-check run/import_type.ts",
  output: "run/import_type.ts.out",
});

itest!(private_field_presence {
  args: "run --reload run/private_field_presence.ts",
  output: "run/private_field_presence.ts.out",
});

itest!(private_field_presence_no_check {
  args: "run --reload --no-check run/private_field_presence.ts",
  output: "run/private_field_presence.ts.out",
});

itest!(lock_write_fetch {
  args:
    "run --quiet --allow-read --allow-write --allow-env --allow-run run/lock_write_fetch/main.ts",
  output: "run/lock_write_fetch/main.out",
  http_server: true,
  exit_code: 0,
});

itest!(lock_check_ok {
  args:
    "run --quiet --lock=run/lock_check_ok.json http://127.0.0.1:4545/run/003_relative_import.ts",
  output: "run/003_relative_import.ts.out",
  http_server: true,
});

itest!(lock_check_ok2 {
  args: "run --lock=run/lock_check_ok2.json run/019_media_types.ts",
  output: "run/019_media_types.ts.out",
  http_server: true,
});

itest!(lock_dynamic_imports {
  args: "run --lock=run/lock_dynamic_imports.json --allow-read --allow-net http://127.0.0.1:4545/run/013_dynamic_import.ts",
  output: "run/lock_dynamic_imports.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err {
  args: "run --lock=run/lock_check_err.json http://127.0.0.1:4545/run/003_relative_import.ts",
  output: "run/lock_check_err.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err2 {
  args: "run --lock=run/lock_check_err2.json run/019_media_types.ts",
  output: "run/lock_check_err2.out",
  exit_code: 10,
  http_server: true,
});

itest!(config_file_lock_path {
  args: "run --config=run/config_file_lock_path.json run/019_media_types.ts",
  output: "run/config_file_lock_path.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_flag_overrides_config_file_lock_path {
  args: "run --lock=run/lock_check_ok2.json --config=run/config_file_lock_path.json run/019_media_types.ts",
  output: "run/019_media_types.ts.out",
  http_server: true,
});

itest!(lock_v2_check_ok {
  args:
    "run --quiet --lock=run/lock_v2_check_ok.json http://127.0.0.1:4545/run/003_relative_import.ts",
  output: "run/003_relative_import.ts.out",
  http_server: true,
});

itest!(lock_v2_check_ok2 {
  args: "run --lock=run/lock_v2_check_ok2.json run/019_media_types.ts",
  output: "run/019_media_types.ts.out",
  http_server: true,
});

itest!(lock_v2_dynamic_imports {
  args: "run --lock=run/lock_v2_dynamic_imports.json --allow-read --allow-net http://127.0.0.1:4545/run/013_dynamic_import.ts",
  output: "run/lock_v2_dynamic_imports.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_v2_check_err {
  args: "run --lock=run/lock_v2_check_err.json http://127.0.0.1:4545/run/003_relative_import.ts",
  output: "run/lock_v2_check_err.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_v2_check_err2 {
  args: "run --lock=run/lock_v2_check_err2.json run/019_media_types.ts",
  output: "run/lock_v2_check_err2.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_only_http_and_https {
  args: "run --lock=run/lock_only_http_and_https/deno.lock run/lock_only_http_and_https/main.ts",
  output: "run/lock_only_http_and_https/main.out",
  http_server: true,
});

#[test]
fn lock_no_declaration_files() {
  let context = TestContextBuilder::new()
    .use_temp_cwd()
    .use_http_server()
    .build();
  let output = context
    .new_command()
    .args("cache --lock --lock-write $TESTDATA/lockfile/no_dts/main.ts")
    .run();
  output.assert_matches_file("lockfile/no_dts/main.cache.out");
  let lockfile = context.temp_dir().path().join("deno.lock");
  lockfile.assert_matches_file("lockfile/no_dts/deno.lock.out");
}

#[test]
fn lock_redirects() {
  let context = TestContextBuilder::new()
    .use_temp_cwd()
    .use_http_server()
    .add_npm_env_vars()
    .build();
  let temp_dir = context.temp_dir();
  temp_dir.write("deno.json", "{}"); // cause a lockfile to be created
  temp_dir.write(
    "main.ts",
    "import 'http://localhost:4546/run/001_hello.js';",
  );
  context
    .new_command()
    .args("run main.ts")
    .run()
    .skip_output_check();
  let initial_lockfile_text = r#"{
  "version": "3",
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/run/001_hello.js"
  },
  "remote": {
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#;
  assert_eq!(temp_dir.read_to_string("deno.lock"), initial_lockfile_text);
  context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text("Hello World\n");
  assert_eq!(temp_dir.read_to_string("deno.lock"), initial_lockfile_text);

  // now try changing where the redirect occurs in the lockfile
  temp_dir.write("deno.lock", r#"{
  "version": "3",
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/echo.ts"
  },
  "remote": {
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#);

  // also, add some npm dependency to ensure it doesn't end up in
  // the redirects as they're currently stored separately
  temp_dir.write(
    "main.ts",
    "import 'http://localhost:4546/run/001_hello.js';\n import 'npm:@denotest/esm-basic';\n",
  );

  // it should use the echo script instead
  context
    .new_command()
    .args("run main.ts Hi there")
    .run()
    .assert_matches_text(
      concat!(
        "Download http://localhost:4545/echo.ts\n",
        "Download http://localhost:4545/npm/registry/@denotest/esm-basic\n",
        "Download http://localhost:4545/npm/registry/@denotest/esm-basic/1.0.0.tgz\n",
        "Hi, there",
    ));
  util::assertions::assert_wildcard_match(
    &temp_dir.read_to_string("deno.lock"),
    r#"{
  "version": "3",
  "packages": {
    "specifiers": {
      "npm:@denotest/esm-basic": "npm:@denotest/esm-basic@1.0.0"
    },
    "npm": {
      "@denotest/esm-basic@1.0.0": {
        "integrity": "sha512-[WILDCARD]",
        "dependencies": {}
      }
    }
  },
  "redirects": {
    "http://localhost:4546/run/001_hello.js": "http://localhost:4545/echo.ts"
  },
  "remote": {
    "http://localhost:4545/echo.ts": "829eb4d67015a695d70b2a33c78b631b29eea1dbac491a6bfcf394af2a2671c2",
    "http://localhost:4545/run/001_hello.js": "c479db5ea26965387423ca438bb977d0b4788d5901efcef52f69871e4c1048c5"
  }
}
"#,
  );
}

itest!(mts_dmts_mjs {
  args: "run subdir/import.mts",
  output: "run/mts_dmts_mjs.out",
});

itest!(mts_dmts_mjs_no_check {
  args: "run --no-check subdir/import.mts",
  output: "run/mts_dmts_mjs.out",
});

itest!(async_error {
  exit_code: 1,
  args: "run --reload run/async_error.ts",
  output: "run/async_error.ts.out",
});

itest!(config {
  args:
    "run --reload --config run/config/tsconfig.json --check run/config/main.ts",
  output: "run/config/main.out",
});

itest!(config_types {
  args:
    "run --reload --quiet --check=all --config run/config_types/tsconfig.json run/config_types/main.ts",
  output: "run/config_types/main.out",
});

itest!(config_types_remote {
  http_server: true,
  args: "run --reload --quiet --check=all --config run/config_types/remote.tsconfig.json run/config_types/main.ts",
  output: "run/config_types/main.out",
});

itest!(empty_typescript {
  args: "run --reload --check run/empty.ts",
  output_str: Some("Check file:[WILDCARD]/run/empty.ts\n"),
});

itest!(error_001 {
  args: "run --reload run/error_001.ts",
  exit_code: 1,
  output: "run/error_001.ts.out",
});

itest!(error_002 {
  args: "run --reload run/error_002.ts",
  exit_code: 1,
  output: "run/error_002.ts.out",
});

itest!(error_003_typescript {
  args: "run --reload --check run/error_003_typescript.ts",
  exit_code: 1,
  output: "run/error_003_typescript.ts.out",
});

// Supposing that we've already attempted to run error_003_typescript.ts
// we want to make sure that JS wasn't emitted. Running again without reload flag
// should result in the same output.
// https://github.com/denoland/deno/issues/2436
itest!(error_003_typescript2 {
  args: "run --check run/error_003_typescript.ts",
  exit_code: 1,
  output: "run/error_003_typescript.ts.out",
});

itest!(error_004_missing_module {
  args: "run --reload run/error_004_missing_module.ts",
  exit_code: 1,
  output: "run/error_004_missing_module.ts.out",
});

itest!(error_005_missing_dynamic_import {
  args:
    "run --reload --allow-read --quiet run/error_005_missing_dynamic_import.ts",
  exit_code: 1,
  output: "run/error_005_missing_dynamic_import.ts.out",
});

itest!(error_006_import_ext_failure {
  args: "run --reload run/error_006_import_ext_failure.ts",
  exit_code: 1,
  output: "run/error_006_import_ext_failure.ts.out",
});

itest!(error_007_any {
  args: "run --reload run/error_007_any.ts",
  exit_code: 1,
  output: "run/error_007_any.ts.out",
});

itest!(error_008_checkjs {
  args: "run --reload run/error_008_checkjs.js",
  exit_code: 1,
  output: "run/error_008_checkjs.js.out",
});

itest!(error_009_extensions_error {
  args: "run run/error_009_extensions_error.js",
  output: "run/error_009_extensions_error.js.out",
  exit_code: 1,
});

itest!(error_011_bad_module_specifier {
  args: "run --reload run/error_011_bad_module_specifier.ts",
  exit_code: 1,
  output: "run/error_011_bad_module_specifier.ts.out",
});

itest!(error_012_bad_dynamic_import_specifier {
  args: "run --reload --check run/error_012_bad_dynamic_import_specifier.ts",
  exit_code: 1,
  output: "run/error_012_bad_dynamic_import_specifier.ts.out",
});

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  exit_code: 1,
  output: "run/error_013_missing_script.out",
});

itest!(error_014_catch_dynamic_import_error {
  args:
    "run  --reload --allow-read run/error_014_catch_dynamic_import_error.js",
  output: "run/error_014_catch_dynamic_import_error.js.out",
});

itest!(error_015_dynamic_import_permissions {
  args: "run --reload --quiet run/error_015_dynamic_import_permissions.js",
  output: "run/error_015_dynamic_import_permissions.out",
  exit_code: 1,
  http_server: true,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args: "run --reload --allow-net run/error_016_dynamic_import_permissions2.js",
  output: "run/error_016_dynamic_import_permissions2.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_017_hide_long_source_ts {
  args: "run --reload --check run/error_017_hide_long_source_ts.ts",
  output: "run/error_017_hide_long_source_ts.ts.out",
  exit_code: 1,
});

itest!(error_018_hide_long_source_js {
  args: "run run/error_018_hide_long_source_js.js",
  output: "run/error_018_hide_long_source_js.js.out",
  exit_code: 1,
});

itest!(error_019_stack_function {
  args: "run run/error_019_stack_function.ts",
  output: "run/error_019_stack_function.ts.out",
  exit_code: 1,
});

itest!(error_020_stack_constructor {
  args: "run run/error_020_stack_constructor.ts",
  output: "run/error_020_stack_constructor.ts.out",
  exit_code: 1,
});

itest!(error_021_stack_method {
  args: "run run/error_021_stack_method.ts",
  output: "run/error_021_stack_method.ts.out",
  exit_code: 1,
});

itest!(error_022_stack_custom_error {
  args: "run run/error_022_stack_custom_error.ts",
  output: "run/error_022_stack_custom_error.ts.out",
  exit_code: 1,
});

itest!(error_023_stack_async {
  args: "run run/error_023_stack_async.ts",
  output: "run/error_023_stack_async.ts.out",
  exit_code: 1,
});

itest!(error_024_stack_promise_all {
  args: "run run/error_024_stack_promise_all.ts",
  output: "run/error_024_stack_promise_all.ts.out",
  exit_code: 1,
});

itest!(error_025_tab_indent {
  args: "run run/error_025_tab_indent",
  output: "run/error_025_tab_indent.out",
  exit_code: 1,
});

itest!(error_026_remote_import_error {
  args: "run run/error_026_remote_import_error.ts",
  output: "run/error_026_remote_import_error.ts.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_for_await {
  args: "run --reload --check run/error_for_await.ts",
  output: "run/error_for_await.ts.out",
  exit_code: 1,
});

itest!(error_missing_module_named_import {
  args: "run --reload run/error_missing_module_named_import.ts",
  output: "run/error_missing_module_named_import.ts.out",
  exit_code: 1,
});

itest!(error_no_check {
  args: "run --reload --no-check run/error_no_check.ts",
  output: "run/error_no_check.ts.out",
  exit_code: 1,
});

itest!(error_syntax {
  args: "run --reload run/error_syntax.js",
  exit_code: 1,
  output: "run/error_syntax.js.out",
});

itest!(error_syntax_empty_trailing_line {
  args: "run --reload run/error_syntax_empty_trailing_line.mjs",
  exit_code: 1,
  output: "run/error_syntax_empty_trailing_line.mjs.out",
});

itest!(error_type_definitions {
  args: "run --reload --check run/error_type_definitions.ts",
  exit_code: 1,
  output: "run/error_type_definitions.ts.out",
});

itest!(error_local_static_import_from_remote_ts {
    args: "run --reload http://localhost:4545/run/error_local_static_import_from_remote.ts",
    exit_code: 1,
    http_server: true,
    output: "run/error_local_static_import_from_remote.ts.out",
  });

itest!(error_local_static_import_from_remote_js {
    args: "run --reload http://localhost:4545/run/error_local_static_import_from_remote.js",
    exit_code: 1,
    http_server: true,
    output: "run/error_local_static_import_from_remote.js.out",
  });

itest!(exit_error42 {
  exit_code: 42,
  args: "run --quiet --reload run/exit_error42.ts",
  output: "run/exit_error42.ts.out",
});

itest!(set_exit_code_0 {
  args: "run --no-check --unstable run/set_exit_code_0.ts",
  output_str: Some(""),
  exit_code: 0,
});

itest!(set_exit_code_1 {
  args: "run --no-check --unstable run/set_exit_code_1.ts",
  output_str: Some(""),
  exit_code: 42,
});

itest!(set_exit_code_2 {
  args: "run --no-check --unstable run/set_exit_code_2.ts",
  output_str: Some(""),
  exit_code: 42,
});

itest!(op_exit_op_set_exit_code_in_worker {
  args: "run --no-check --unstable --allow-read run/op_exit_op_set_exit_code_in_worker.ts",
  exit_code: 21,
  output_str: Some(""),
});

itest!(deno_exit_tampering {
  args: "run --no-check --unstable run/deno_exit_tampering.ts",
  output_str: Some(""),
  exit_code: 42,
});

itest!(heapstats {
  args: "run --quiet --unstable --v8-flags=--expose-gc run/heapstats.js",
  output: "run/heapstats.js.out",
});

itest!(finalization_registry {
  args:
    "run --quiet --unstable --v8-flags=--expose-gc run/finalization_registry.js",
  output: "run/finalization_registry.js.out",
});

itest!(https_import {
  args: "run --quiet --reload --cert tls/RootCA.pem run/https_import.ts",
  output: "run/https_import.ts.out",
  http_server: true,
});

itest!(if_main {
  args: "run --quiet --reload run/if_main.ts",
  output: "run/if_main.ts.out",
});

itest!(import_meta {
  args: "run --quiet --reload --import-map=run/import_meta/importmap.json run/import_meta/main.ts",
  output: "run/import_meta/main.out",
});

itest!(main_module {
  args: "run --quiet --allow-read --reload run/main_module/main.ts",
  output: "run/main_module/main.out",
});

itest!(no_check {
  args: "run --quiet --reload --no-check run/006_url_imports.ts",
  output: "run/006_url_imports.ts.out",
  http_server: true,
});

itest!(no_check_decorators {
  args: "run --quiet --reload --no-check run/no_check_decorators.ts",
  output: "run/no_check_decorators.ts.out",
});

itest!(check_remote {
  args: "run --quiet --reload --check=all run/no_check_remote.ts",
  output: "run/no_check_remote.ts.disabled.out",
  exit_code: 1,
  http_server: true,
});

itest!(no_check_remote {
  args: "run --quiet --reload --no-check=remote run/no_check_remote.ts",
  output: "run/no_check_remote.ts.enabled.out",
  http_server: true,
});

itest!(runtime_decorators {
  args: "run --quiet --reload --no-check run/runtime_decorators.ts",
  output: "run/runtime_decorators.ts.out",
});

itest!(seed_random {
  args: "run --seed=100 run/seed_random.js",
  output: "run/seed_random.js.out",
});

itest!(type_definitions {
  args: "run --reload run/type_definitions.ts",
  output: "run/type_definitions.ts.out",
});

itest!(type_definitions_for_export {
  args: "run --reload --check run/type_definitions_for_export.ts",
  output: "run/type_definitions_for_export.ts.out",
  exit_code: 1,
});

itest!(type_directives_01 {
  args: "run --reload --check=all -L debug run/type_directives_01.ts",
  output: "run/type_directives_01.ts.out",
  http_server: true,
});

itest!(type_directives_02 {
  args: "run --reload --check=all -L debug run/type_directives_02.ts",
  output: "run/type_directives_02.ts.out",
});

#[test]
fn type_directives_js_main() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("run --reload -L debug --check run/type_directives_js_main.js")
    .run();
  output.assert_matches_text("[WILDCARD] - FileFetcher::fetch() - specifier: file:///[WILDCARD]/subdir/type_reference.d.ts[WILDCARD]");
  let output = context
    .new_command()
    .args("run --reload -L debug run/type_directives_js_main.js")
    .run();
  assert_not_contains!(output.combined_output(), "type_reference.d.ts");
}

itest!(type_directives_redirect {
  args: "run --reload --check run/type_directives_redirect.ts",
  output: "run/type_directives_redirect.ts.out",
  http_server: true,
});

itest!(type_headers_deno_types {
  args: "run --reload --check run/type_headers_deno_types.ts",
  output: "run/type_headers_deno_types.ts.out",
  http_server: true,
});

itest!(ts_type_imports {
  args: "run --reload --check run/ts_type_imports.ts",
  output: "run/ts_type_imports.ts.out",
  exit_code: 1,
});

itest!(ts_decorators {
  args: "run --reload --check run/ts_decorators.ts",
  output: "run/ts_decorators.ts.out",
});

itest!(ts_type_only_import {
  args: "run --reload --check run/ts_type_only_import.ts",
  output: "run/ts_type_only_import.ts.out",
});

itest!(swc_syntax_error {
  args: "run --reload --check run/swc_syntax_error.ts",
  output: "run/swc_syntax_error.ts.out",
  exit_code: 1,
});

itest!(unbuffered_stderr {
  args: "run --reload run/unbuffered_stderr.ts",
  output: "run/unbuffered_stderr.ts.out",
});

itest!(unbuffered_stdout {
  args: "run --quiet --reload run/unbuffered_stdout.ts",
  output: "run/unbuffered_stdout.ts.out",
});

itest!(v8_flags_run {
  args: "run --v8-flags=--expose-gc run/v8_flags.js",
  output: "run/v8_flags.js.out",
});

itest!(v8_flags_env_run {
  envs: vec![("DENO_V8_FLAGS".to_string(), "--expose-gc".to_string())],
  args: "run run/v8_flags.js",
  output: "run/v8_flags.js.out",
});

itest!(v8_flags_unrecognized {
  args: "repl --v8-flags=--foo,bar,--trace-gc,-baz",
  output: "run/v8_flags_unrecognized.out",
  exit_code: 1,
});

itest!(v8_help {
  args: "repl --v8-flags=--help",
  output: "run/v8_help.out",
});

itest!(unsupported_dynamic_import_scheme {
  args: "eval import('xxx:')",
  output: "run/unsupported_dynamic_import_scheme.out",
  exit_code: 1,
});

itest!(wasm {
  args: "run --quiet run/wasm.ts",
  output: "run/wasm.ts.out",
});

itest!(wasm_shared {
  args: "run --quiet run/wasm_shared.ts",
  output: "run/wasm_shared.out",
});

itest!(wasm_async {
  args: "run run/wasm_async.js",
  output: "run/wasm_async.out",
});

itest!(wasm_unreachable {
  args: "run --allow-read run/wasm_unreachable.js",
  output: "run/wasm_unreachable.out",
  exit_code: 1,
});

itest!(wasm_url {
  args: "run --quiet --allow-net=localhost:4545 run/wasm_url.js",
  output: "run/wasm_url.out",
  exit_code: 1,
  http_server: true,
});

itest!(weakref {
  args: "run --quiet --reload run/weakref.ts",
  output: "run/weakref.ts.out",
});

itest!(top_level_await_order {
  args: "run --allow-read run/top_level_await/order.js",
  output: "run/top_level_await/order.out",
});

itest!(top_level_await_loop {
  args: "run --allow-read run/top_level_await/loop.js",
  output: "run/top_level_await/loop.out",
});

itest!(top_level_await_circular {
  args: "run --allow-read run/top_level_await/circular.js",
  output: "run/top_level_await/circular.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/11238.
itest!(top_level_await_nested {
  args: "run --allow-read run/top_level_await/nested/main.js",
  output: "run/top_level_await/nested.out",
});

itest!(top_level_await_unresolved {
  args: "run run/top_level_await/unresolved.js",
  output: "run/top_level_await/unresolved.out",
  exit_code: 1,
});

itest!(top_level_await {
  args: "run --allow-read run/top_level_await/top_level_await.js",
  output: "run/top_level_await/top_level_await.out",
});

itest!(top_level_await_ts {
  args: "run --quiet --allow-read run/top_level_await/top_level_await.ts",
  output: "run/top_level_await/top_level_await.out",
});

itest!(top_level_for_await {
  args: "run --quiet run/top_level_await/top_level_for_await.js",
  output: "run/top_level_await/top_level_for_await.out",
});

itest!(top_level_for_await_ts {
  args: "run --quiet run/top_level_await/top_level_for_await.ts",
  output: "run/top_level_await/top_level_for_await.out",
});

itest!(unstable_disabled {
  args: "run --reload --check run/unstable.ts",
  exit_code: 1,
  output: "run/unstable_disabled.out",
});

itest!(unstable_enabled {
  args: "run --quiet --reload --unstable run/unstable.ts",
  output: "run/unstable_enabled.out",
});

itest!(unstable_disabled_js {
  args: "run --reload run/unstable.js",
  output: "run/unstable_disabled_js.out",
});

itest!(unstable_enabled_js {
  args: "run --quiet --reload --unstable run/unstable.ts",
  output: "run/unstable_enabled_js.out",
});

itest!(unstable_worker {
  args: "run --reload --unstable --quiet --allow-read run/unstable_worker.ts",
  output: "run/unstable_worker.ts.out",
});

itest!(unstable_worker_options_disabled {
  args: "run --quiet --reload --allow-read run/unstable_worker_options.js",
  output: "run/unstable_worker_options.disabled.out",
  exit_code: 70,
});

itest!(unstable_worker_options_enabled {
  args: "run --quiet --reload --allow-read --unstable-worker-options run/unstable_worker_options.js",
  output: "run/unstable_worker_options.enabled.out",
});

itest!(unstable_broadcast_channel_disabled {
  args: "run --quiet --reload --allow-read run/unstable_broadcast_channel.js",
  output: "run/unstable_broadcast_channel.disabled.out",
});

itest!(unstable_broadcast_channel_enabled {
  args: "run --quiet --reload --allow-read --unstable-broadcast-channel run/unstable_broadcast_channel.js",
  output: "run/unstable_broadcast_channel.enabled.out",
});

itest!(unstable_cron_disabled {
  args: "run --quiet --reload --allow-read run/unstable_cron.js",
  output: "run/unstable_cron.disabled.out",
});

itest!(unstable_cron_enabled {
  args:
    "run --quiet --reload --allow-read --unstable-cron run/unstable_cron.js",
  output: "run/unstable_cron.enabled.out",
});

itest!(unstable_ffi_disabled {
  args: "run --quiet --reload --allow-read run/unstable_ffi.js",
  output: "run/unstable_ffi.disabled.out",
});

itest!(unstable_ffi_enabled {
  args: "run --quiet --reload --allow-read --unstable-ffi run/unstable_ffi.js",
  output: "run/unstable_ffi.enabled.out",
});

itest!(unstable_fs_disabled {
  args: "run --quiet --reload --allow-read run/unstable_fs.js",
  output: "run/unstable_fs.disabled.out",
});

itest!(unstable_fs_enabled {
  args: "run --quiet --reload --allow-read --unstable-fs run/unstable_fs.js",
  output: "run/unstable_fs.enabled.out",
});

itest!(unstable_http_disabled {
  args: "run --quiet --reload --allow-read run/unstable_http.js",
  output: "run/unstable_http.disabled.out",
});

itest!(unstable_http_enabled {
  args:
    "run --quiet --reload --allow-read --unstable-http run/unstable_http.js",
  output: "run/unstable_http.enabled.out",
});

itest!(unstable_net_disabled {
  args: "run --quiet --reload --allow-read run/unstable_net.js",
  output: "run/unstable_net.disabled.out",
});

itest!(unstable_net_enabled {
  args: "run --quiet --reload --allow-read --unstable-net run/unstable_net.js",
  output: "run/unstable_net.enabled.out",
});

itest!(unstable_kv_disabled {
  args: "run --quiet --reload --allow-read run/unstable_kv.js",
  output: "run/unstable_kv.disabled.out",
});

itest!(unstable_kv_enabled {
  args: "run --quiet --reload --allow-read --unstable-kv run/unstable_kv.js",
  output: "run/unstable_kv.enabled.out",
});

itest!(unstable_webgpu_disabled {
  args: "run --quiet --reload --allow-read run/unstable_webgpu.js",
  output: "run/unstable_webgpu.disabled.out",
});

itest!(unstable_webgpu_enabled {
  args:
    "run --quiet --reload --allow-read --unstable-webgpu run/unstable_webgpu.js",
  output: "run/unstable_webgpu.enabled.out",
});

itest!(import_compression {
  args: "run --quiet --reload --allow-net run/import_compression/main.ts",
  output: "run/import_compression/main.out",
  http_server: true,
});

itest!(disallow_http_from_https_js {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/run/disallow_http_from_https.js",
  output: "run/disallow_http_from_https_js.out",
  http_server: true,
  exit_code: 1,
});

itest!(disallow_http_from_https_ts {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/run/disallow_http_from_https.ts",
  output: "run/disallow_http_from_https_ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(dynamic_import_conditional {
  args: "run --quiet --reload run/dynamic_import_conditional.js",
  output: "run/dynamic_import_conditional.js.out",
});

itest!(tsx_imports {
  args: "run --reload --check run/tsx_imports/tsx_imports.ts",
  output: "run/tsx_imports/tsx_imports.ts.out",
});

itest!(fix_dynamic_import_errors {
  args: "run --reload run/fix_dynamic_import_errors.js",
  output: "run/fix_dynamic_import_errors.js.out",
});

itest!(fix_emittable_skipped {
  args: "run --reload run/fix_emittable_skipped.js",
  output: "run/fix_emittable_skipped.ts.out",
});

itest!(fix_js_import_js {
  args: "run --quiet --reload run/fix_js_import_js.ts",
  output: "run/fix_js_import_js.ts.out",
});

itest!(fix_js_imports {
  args: "run --quiet --reload run/fix_js_imports.ts",
  output: "run/fix_js_imports.ts.out",
});

itest!(fix_tsc_file_exists {
  args: "run --quiet --reload tsc/test.js",
  output: "run/fix_tsc_file_exists.out",
});

itest!(fix_worker_dispatchevent {
  args: "run --quiet --reload run/fix_worker_dispatchevent.ts",
  output: "run/fix_worker_dispatchevent.ts.out",
});

itest!(es_private_fields {
  args: "run --quiet --reload run/es_private_fields.js",
  output: "run/es_private_fields.js.out",
});

itest!(cjs_imports {
  args: "run --quiet --reload run/cjs_imports/main.ts",
  output: "run/cjs_imports/main.out",
});

itest!(ts_import_from_js {
  args: "run --quiet --reload run/ts_import_from_js/main.js",
  output: "run/ts_import_from_js/main.out",
  http_server: true,
});

itest!(jsx_import_from_ts {
  args: "run --quiet --reload run/jsx_import_from_ts.ts",
  output: "run/jsx_import_from_ts.ts.out",
});

itest!(jsx_import_source_pragma {
  args: "run --reload run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_config {
  args:
    "run --reload --config jsx/deno-jsx.jsonc --no-lock run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_dev_config {
  args:
    "run --reload --config jsx/deno-jsxdev.jsonc --no-lock run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source_dev.out",
  http_server: true,
});

itest!(jsx_import_source_no_pragma {
  args:
    "run --reload --config jsx/deno-jsx.jsonc --no-lock run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_no_pragma_dev {
  args: "run --reload --config jsx/deno-jsxdev.jsonc --no-lock run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_dev.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_import_map {
  args: "run --reload --import-map jsx/import-map.json run/jsx_import_source_pragma_import_map.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_import_map_dev {
  args: "run --reload --import-map jsx/import-map.json --config jsx/deno-jsxdev-import-map.jsonc run/jsx_import_source_pragma_import_map.tsx",
  output: "run/jsx_import_source_import_map_dev.out",
  http_server: true,
});

itest!(jsx_import_source_precompile_import_map {
  args: "run --reload --check --import-map jsx/import-map.json --no-lock --config jsx/deno-jsx-precompile.jsonc run/jsx_precompile/no_pragma.tsx",
  output: "run/jsx_precompile/no_pragma.out",
  http_server: true,
});

itest!(jsx_import_source_import_map {
  args: "run --reload --import-map jsx/import-map.json --no-lock --config jsx/deno-jsx-import-map.jsonc run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_dev {
  args: "run --reload --import-map jsx/import-map.json --no-lock --config jsx/deno-jsxdev-import-map.jsonc run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map_dev.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_scoped {
  args: "run --reload --import-map jsx/import-map-scoped.json --no-lock --config jsx/deno-jsx-import-map.jsonc subdir/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_scoped_dev {
  args: "run --reload --import-map jsx/import-map-scoped.json --no-lock --config jsx/deno-jsxdev-import-map.jsonc subdir/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map_dev.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_no_check {
  args: "run --reload --no-check run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_config_no_check {
  args: "run --reload --config jsx/deno-jsx.jsonc --no-lock --no-check run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_with_config_vendor_dir {
  args: "run --reload --config jsx/deno-jsx.jsonc --no-lock --vendor $TESTDATA/run/jsx_import_source_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
  temp_cwd: true,
  copy_temp_dir: Some("jsx/"),
});

itest!(jsx_import_source_no_pragma_no_check {
  args:
    "run --reload --config jsx/deno-jsx.jsonc --no-lock --no-check run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source.out",
  http_server: true,
});

itest!(jsx_import_source_pragma_import_map_no_check {
  args: "run --reload --import-map jsx/import-map.json --no-check run/jsx_import_source_pragma_import_map.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_import_map_no_check {
  args: "run --reload --import-map jsx/import-map.json --no-lock --config jsx/deno-jsx-import-map.jsonc --no-check run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_import_map.out",
  http_server: true,
});

itest!(jsx_import_source_error {
  args: "run --config jsx/deno-jsx-error.jsonc --check run/jsx_import_source_no_pragma.tsx",
  output: "run/jsx_import_source_error.out",
  exit_code: 1,
});

itest!(single_compile_with_reload {
  args: "run --reload --allow-read run/single_compile_with_reload.ts",
  output: "run/single_compile_with_reload.ts.out",
});

itest!(proto_exploit {
  args: "run run/proto_exploit.js",
  output: "run/proto_exploit.js.out",
});

itest!(reference_types {
  args: "run --reload --quiet run/reference_types.ts",
  output: "run/reference_types.ts.out",
});

itest!(references_types_remote {
  http_server: true,
  args: "run --reload --quiet run/reference_types_remote.ts",
  output: "run/reference_types_remote.ts.out",
});

itest!(reference_types_error {
  args:
    "run --config run/checkjs.tsconfig.json --check run/reference_types_error.js",
  output: "run/reference_types_error.js.out",
  exit_code: 1,
});

itest!(reference_types_error_vendor_dir {
  args:
    "run --config run/checkjs.tsconfig.json --check --vendor $TESTDATA/run/reference_types_error.js",
  output: "run/reference_types_error.js.out",
  exit_code: 1,
});

itest!(reference_types_error_no_check {
  args: "run --no-check run/reference_types_error.js",
  output_str: Some(""),
});

itest!(import_data_url_error_stack {
  args: "run --quiet --reload run/import_data_url_error_stack.ts",
  output: "run/import_data_url_error_stack.ts.out",
  exit_code: 1,
});

itest!(import_data_url_import_relative {
  args: "run --quiet --reload run/import_data_url_import_relative.ts",
  output: "run/import_data_url_import_relative.ts.out",
  exit_code: 1,
});

itest!(import_data_url_import_map {
    args: "run --quiet --reload --import-map import_maps/import_map.json run/import_data_url.ts",
    output: "run/import_data_url.ts.out",
  });

itest!(import_data_url_imports {
  args: "run --quiet --reload run/import_data_url_imports.ts",
  output: "run/import_data_url_imports.ts.out",
  http_server: true,
});

itest!(import_data_url_jsx {
  args: "run --quiet --reload run/import_data_url_jsx.ts",
  output: "run/import_data_url_jsx.ts.out",
});

itest!(import_data_url {
  args: "run --quiet --reload run/import_data_url.ts",
  output: "run/import_data_url.ts.out",
});

itest!(import_dynamic_data_url {
  args: "run --quiet --reload run/import_dynamic_data_url.ts",
  output: "run/import_dynamic_data_url.ts.out",
});

itest!(import_blob_url_error_stack {
  args: "run --quiet --reload run/import_blob_url_error_stack.ts",
  output: "run/import_blob_url_error_stack.ts.out",
  exit_code: 1,
});

itest!(import_blob_url_import_relative {
  args: "run --quiet --reload run/import_blob_url_import_relative.ts",
  output: "run/import_blob_url_import_relative.ts.out",
  exit_code: 1,
});

itest!(import_blob_url_imports {
  args:
    "run --quiet --reload --allow-net=localhost:4545 run/import_blob_url_imports.ts",
  output: "run/import_blob_url_imports.ts.out",
  http_server: true,
});

itest!(import_blob_url_jsx {
  args: "run --quiet --reload run/import_blob_url_jsx.ts",
  output: "run/import_blob_url_jsx.ts.out",
});

itest!(import_blob_url {
  args: "run --quiet --reload run/import_blob_url.ts",
  output: "run/import_blob_url.ts.out",
});

itest!(import_file_with_colon {
  args: "run --quiet --reload run/import_file_with_colon.ts",
  output: "run/import_file_with_colon.ts.out",
  http_server: true,
});

itest!(import_extensionless {
  args: "run --quiet --reload run/import_extensionless.ts",
  output: "run/import_extensionless.ts.out",
  http_server: true,
});

itest!(classic_workers_event_loop {
  args:
    "run --enable-testing-features-do-not-use run/classic_workers_event_loop.js",
  output: "run/classic_workers_event_loop.js.out",
});

// FIXME(bartlomieju): disabled, because this test is very flaky on CI
// itest!(local_sources_not_cached_in_memory {
//   args: "run --allow-read --allow-write run/no_mem_cache.js",
//   output: "run/no_mem_cache.js.out",
// });

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, but is not loaded into the module
// graph (inline_js_source_map_2.ts) (because there are no direct dependencies).
// Source line is not remapped because no inline source contents are included in
// the sourcemap and the file is not present in the dependency graph.
itest!(inline_js_source_map_2 {
  args: "run --quiet run/inline_js_source_map_2.js",
  output: "run/inline_js_source_map_2.js.out",
  exit_code: 1,
});

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, but is not loaded into the module
// graph (inline_js_source_map_2.ts) (because there are no direct dependencies).
// Source line remapped using th inline source contents that are included in the
// inline source map.
itest!(inline_js_source_map_2_with_inline_contents {
  args: "run --quiet run/inline_js_source_map_2_with_inline_contents.js",
  output: "run/inline_js_source_map_2_with_inline_contents.js.out",
  exit_code: 1,
});

// This test checks that inline source map data is used. It uses a hand crafted
// source map that maps to a file that exists, and is loaded into the module
// graph because of a direct import statement (inline_js_source_map.ts). The
// source map was generated from an earlier version of this file, where the throw
// was not commented out. The source line is remapped using source contents that
// from the module graph.
itest!(inline_js_source_map_with_contents_from_graph {
  args: "run --quiet run/inline_js_source_map_with_contents_from_graph.js",
  output: "run/inline_js_source_map_with_contents_from_graph.js.out",
  exit_code: 1,
  http_server: true,
});

// This test ensures that a descriptive error is shown when we're unable to load
// the import map. Even though this tests only the `run` subcommand, we can be sure
// that the error message is similar for other subcommands as they all use
// `program_state.maybe_import_map` to access the import map underneath.
itest!(error_import_map_unable_to_load {
  args: "run --import-map=import_maps/does_not_exist.json import_maps/test.ts",
  output: "run/error_import_map_unable_to_load.out",
  exit_code: 1,
});

// Test that setting `self` in the main thread to some other value doesn't break
// the world.
itest!(replace_self {
  args: "run run/replace_self.js",
  output: "run/replace_self.js.out",
});

itest!(worker_event_handler_test {
  args: "run --quiet --reload --allow-read run/worker_event_handler_test.js",
  output: "run/worker_event_handler_test.js.out",
});

itest!(worker_close_race {
  args: "run --quiet --reload --allow-read run/worker_close_race.js",
  output: "run/worker_close_race.js.out",
});

itest!(worker_drop_handle_race {
  args: "run --quiet --reload --allow-read run/worker_drop_handle_race.js",
  output: "run/worker_drop_handle_race.js.out",
  exit_code: 1,
});

itest!(worker_drop_handle_race_terminate {
  args: "run --unstable run/worker_drop_handle_race_terminate.js",
  output: "run/worker_drop_handle_race_terminate.js.out",
});

itest!(worker_close_nested {
  args: "run --quiet --reload --allow-read run/worker_close_nested.js",
  output: "run/worker_close_nested.js.out",
});

itest!(worker_message_before_close {
  args: "run --quiet --reload --allow-read run/worker_message_before_close.js",
  output: "run/worker_message_before_close.js.out",
});

itest!(worker_close_in_wasm_reactions {
  args:
    "run --quiet --reload --allow-read run/worker_close_in_wasm_reactions.js",
  output: "run/worker_close_in_wasm_reactions.js.out",
});

itest!(shebang_tsc {
  args: "run --quiet --check run/shebang.ts",
  output: "run/shebang.ts.out",
});

itest!(shebang_swc {
  args: "run --quiet run/shebang.ts",
  output: "run/shebang.ts.out",
});

itest!(shebang_with_json_imports_tsc {
  args: "run --quiet import_attributes/json_with_shebang.ts",
  output: "import_attributes/json_with_shebang.ts.out",
  exit_code: 1,
});

itest!(shebang_with_json_imports_swc {
  args: "run --quiet --no-check import_attributes/json_with_shebang.ts",
  output: "import_attributes/json_with_shebang.ts.out",
  exit_code: 1,
});

#[test]
fn no_validate_asm() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("run/no_validate_asm.js")
    .piped_output()
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
    .arg("run/exec_path.ts")
    .stdout(Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  let actual = PathRef::new(std::path::Path::new(stdout_str)).canonicalize();
  let expected = util::deno_exe_path().canonicalize();
  assert_eq!(expected, actual);
}

#[test]
fn run_from_stdin_defaults_to_ts() {
  let source_code = r#"
interface Lollipop {
  _: number;
}
console.log("executing typescript");
"#;

  let mut p = util::deno_cmd()
    .arg("run")
    .arg("--check")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let result = p.wait_with_output().unwrap();
  assert!(result.status.success());
  let stdout_str = std::str::from_utf8(&result.stdout).unwrap().trim();
  assert_eq!(stdout_str, "executing typescript");
}

#[test]
fn run_from_stdin_ext() {
  let source_code = r#"
let i = 123;
i = "hello"
console.log("executing javascript");
"#;

  let mut p = util::deno_cmd()
    .args("run --ext js --check -")
    .stdin(std::process::Stdio::piped())
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let result = p.wait_with_output().unwrap();
  assert!(result.status.success());
  let stdout_str = std::str::from_utf8(&result.stdout).unwrap().trim();
  assert_eq!(stdout_str, "executing javascript");
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
  script_path: test_util::PathRef,
  constraints: WinProcConstraints,
) -> Result<(), i64> {
  let file_path = "assets/DenoWinRunner.ps1";
  let constraints = match constraints {
    WinProcConstraints::NoStdIn => "1",
    WinProcConstraints::NoStdOut => "2",
    WinProcConstraints::NoStdErr => "4",
  };
  let deno_exe_path = util::deno_exe_path().to_string();
  let deno_script_path = script_path.to_string();
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
    .arg("run/001_hello.js")
    .stderr_piped()
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
    .arg("run/001_hello.js")
    .env("RUST_LOG", "debug")
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  assert!(!output.stderr.is_empty());
}

#[test]
fn dont_cache_on_check_fail() {
  let context = TestContext::default();
  let output = context
    .new_command()
    .args("run --check=all --reload run/error_003_typescript.ts")
    .split_output()
    .run();
  assert!(!output.stderr().is_empty());
  output.skip_stdout_check();
  output.assert_exit_code(1);

  let output = context
    .new_command()
    .args("run --check=all run/error_003_typescript.ts")
    .split_output()
    .run();
  assert!(!output.stderr().is_empty());
  output.skip_stdout_check();
  output.assert_exit_code(1);
}

mod permissions {
  use test_util as util;
  use util::TestContext;

  // TODO(bartlomieju): remove --unstable once Deno.Command is stabilized
  #[test]
  fn with_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg("--unstable")
        .arg(format!("--allow-{permission}"))
        .arg("run/permission_test.ts")
        .arg(format!("{permission}Required"))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
      assert!(status.success());
    }
  }

  // TODO(bartlomieju): remove --unstable once Deno.Command is stabilized
  #[test]
  fn without_allow() {
    for permission in &util::PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!("run --unstable run/permission_test.ts {permission}Required"),
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
        ))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
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
          "run --allow-{0}={1} run/complex_permissions_test.ts {0} {2}",
          permission,
          util::testdata_path(),
          util::root_path().join("Cargo.toml"),
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
          util::testdata_path(),
        ))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
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
    let test_dir = util::testdata_path();
    let js_dir = util::root_path().join("js");
    for permission in &PERMISSION_VARIANTS {
      let (_, err) = util::run_and_collect_output(
        false,
        &format!(
          "run --allow-{0}={1},{2} run/complex_permissions_test.ts {0} {3}",
          permission,
          test_dir,
          js_dir,
          util::root_path().join("Cargo.toml"),
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
    let test_dir = util::testdata_path();
    let js_dir = util::root_path().join("js");
    for permission in &PERMISSION_VARIANTS {
      let status = util::deno_cmd()
        .current_dir(&util::testdata_path())
        .arg("run")
        .arg(format!("--allow-{permission}={test_dir},{js_dir}"))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
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
        .arg(format!("--allow-{permission}=."))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
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
        .arg(format!("--allow-{permission}=tls/../"))
        .arg("run/complex_permissions_test.ts")
        .arg(permission)
        .arg("run/complex_permissions_test.ts")
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
        "run --allow-net=localhost:4545 run/complex_permissions_test.ts netFetch http://localhost:4545/",
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
        "run --allow-net=deno.land run/complex_permissions_test.ts netFetch http://localhost:4545/",
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
        "run --allow-net=localhost:4545 run/complex_permissions_test.ts netFetch http://localhost:4546/",
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
        "run --allow-net=localhost run/complex_permissions_test.ts netFetch http://localhost:4545/ http://localhost:4546/ http://localhost:4547/",
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
        "run --allow-net=127.0.0.1:4545 run/complex_permissions_test.ts netConnect 127.0.0.1:4545",
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
        "run --allow-net=deno.land run/complex_permissions_test.ts netConnect 127.0.0.1:4546",
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
        "run --allow-net=127.0.0.1:4545 run/complex_permissions_test.ts netConnect 127.0.0.1:4546",
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
        "run --allow-net=127.0.0.1 run/complex_permissions_test.ts netConnect 127.0.0.1:4545 127.0.0.1:4546 127.0.0.1:4547",
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
        "run --allow-net=localhost:4558 run/complex_permissions_test.ts netListen localhost:4558",
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
        "run --allow-net=deno.land run/complex_permissions_test.ts netListen localhost:4545",
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
        "run --allow-net=localhost:4555 run/complex_permissions_test.ts netListen localhost:4556",
        None,
        None,
        false,
      );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn net_listen_allow_localhost() {
    // Port 4600 is chosen to not collide with those used by
    // target/debug/test_server
    let (_, err) = util::run_and_collect_output(
      true,
        "run --allow-net=localhost run/complex_permissions_test.ts netListen localhost:4600",
        None,
        None,
        false,
      );
    assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
  }

  #[test]
  fn _061_permissions_request() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/061_permissions_request.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access to \"foo\".\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("y");
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access to \"bar\".\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("n");
        console.expect("granted");
        console.expect("prompt");
        console.expect("denied");
      });
  }

  #[test]
  fn _061_permissions_request_sync() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/061_permissions_request_sync.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access to \"foo\".\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("y");
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access to \"bar\".\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("n");
        console.expect("granted");
        console.expect("prompt");
        console.expect("denied");
      });
  }

  #[test]
  fn _062_permissions_request_global() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/062_permissions_request_global.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access.\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("y\n");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
      });
  }

  #[test]
  fn _062_permissions_request_global_sync() {
    TestContext::default()
      .new_command()
      .args_vec(["run", "--quiet", "run/062_permissions_request_global_sync.ts"])
      .with_pty(|mut console| {
        console.expect(concat!(
          "┌ ⚠️  Deno requests read access.\r\n",
          "├ Requested by `Deno.permissions.request()` API.\r\n",
          "├ Run again with --allow-read to bypass this prompt.\r\n",
          "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all read permissions)",
        ));
        console.write_line_raw("y");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
        console
          .expect("PermissionStatus { state: \"granted\", onchange: null }");
      });
  }

  itest!(_063_permissions_revoke {
    args: "run --allow-read=foo,bar run/063_permissions_revoke.ts",
    output: "run/063_permissions_revoke.ts.out",
  });

  itest!(_063_permissions_revoke_sync {
    args: "run --allow-read=foo,bar run/063_permissions_revoke_sync.ts",
    output: "run/063_permissions_revoke.ts.out",
  });

  itest!(_064_permissions_revoke_global {
    args: "run --allow-read=foo,bar run/064_permissions_revoke_global.ts",
    output: "run/064_permissions_revoke_global.ts.out",
  });

  itest!(_064_permissions_revoke_global_sync {
    args: "run --allow-read=foo,bar run/064_permissions_revoke_global_sync.ts",
    output: "run/064_permissions_revoke_global.ts.out",
  });

  itest!(_065_permissions_revoke_net {
    args: "run --allow-net run/065_permissions_revoke_net.ts",
    output: "run/065_permissions_revoke_net.ts.out",
  });

  #[test]
  fn _066_prompt() {
    TestContext::default()
      .new_command()
      .args_vec(["repl"])
      .with_pty(|mut console| {
        // alert with no message displays default "Alert"
        // alert displays "[Press any key to continue]"
        // alert can be closed with Enter key
        console.write_line_raw("alert()");
        console.expect("Alert [Press any key to continue]");
        console.write_raw("\r"); // Enter
        console.expect("undefined");

        // alert can be closed with Escape key
        console.write_line_raw("alert()");
        console.expect("Alert [Press any key to continue]");
        console.write_raw("\x1b"); // Escape
        console.expect("undefined");

        // alert can display custom text
        // alert can be closed with arbitrary keyboard key (x)
        if !cfg!(windows) {
          // it seems to work on windows, just not in the tests
          console.write_line_raw("alert('foo')");
          console.expect("foo [Press any key to continue]");
          console.write_raw("x");
          console.expect("undefined");
        }

        // confirm with no message displays default "Confirm"
        // confirm returns true by immediately pressing Enter
        console.write_line_raw("confirm()");
        console.expect("Confirm [Y/n]");
        console.write_raw("\r"); // Enter
        console.expect("true");

        // tese seem to work on windows, just not in the tests
        if !cfg!(windows) {
          // confirm returns false by pressing Escape
          console.write_line_raw("confirm()");
          console.expect("Confirm [Y/n]");
          console.write_raw("\x1b"); // Escape
          console.expect("false");

          // confirm can display custom text
          // confirm returns true by pressing y
          console.write_line_raw("confirm('continue?')");
          console.expect("continue? [Y/n]");
          console.write_raw("y");
          console.expect("true");

          // confirm returns false by pressing n
          console.write_line_raw("confirm('continue?')");
          console.expect("continue? [Y/n]");
          console.write_raw("n");
          console.expect("false");

          // confirm can display custom text
          // confirm returns true by pressing Y
          console.write_line_raw("confirm('continue?')");
          console.expect("continue? [Y/n]");
          console.write_raw("Y");
          console.expect("true");

          // confirm returns false by pressing N
          console.write_line_raw("confirm('continue?')");
          console.expect("continue? [Y/n]");
          console.write_raw("N");
          console.expect("false");
        }

        // prompt with no message displays default "Prompt"
        // prompt returns user-inserted text
        console.write_line_raw("prompt()");
        console.expect("Prompt ");
        console.write_line_raw("abc");
        console.expect("\"abc\"");

        // prompt can display custom text
        // prompt with no default value returns empty string when immediately pressing Enter
        console.write_line_raw("prompt('foo')");
        console.expect("foo ");
        console.write_raw("\r"); // Enter
        console.expect("\"\"");

        // prompt with non-string default value converts it to string
        console.write_line_raw("prompt('foo', 1)");
        console.expect("foo 1");
        console.write_raw("\r"); // Enter
        console.expect("\"1\"");

        // prompt with non-string default value that can't be converted throws an error
        console.write_line_raw("prompt('foo', Symbol())");
        console.expect(
          "Uncaught TypeError: Cannot convert a Symbol value to a string",
        );

        // prompt with empty-string default value returns empty string when immediately pressing Enter
        console.write_line_raw("prompt('foo', '')");
        console.expect("foo ");
        console.write_raw("\r"); // Enter
        console.expect("\"\"");

        // prompt with contentful default value returns default value when immediately pressing Enter
        console.write_line_raw("prompt('foo', 'bar')");
        console.expect("foo bar");
        console.write_raw("\r"); // Enter
        console.expect("\"bar\"");

        // prompt with contentful default value allows editing of default value
        console.write_line_raw("prompt('foo', 'bar')");
        console.expect("foo bar");
        console.write_raw("\x1b[D"); // Left arrow
        console.write_raw("\x1b[D"); // Left arrow
        console.write_raw("\x7f"); // Backspace
        console.write_raw("c");
        console.expect("foo car");
        console.write_raw("\r"); // Enter
        console.expect("\"car\"");

        // prompt returns null by pressing Escape
        console.write_line_raw("prompt()");
        console.expect("Prompt ");
        console.write_raw("\x1b"); // Escape
        console.expect("null");

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
          // confirm returns false by pressing Ctrl+C
          console.write_line_raw("confirm()");
          console.expect("Confirm [Y/n] ");
          console.write_raw("\x03"); // Ctrl+C
          console.expect("false");

          // confirm returns false by pressing Ctrl+D
          console.write_line_raw("confirm()");
          console.expect("Confirm [Y/n] ");
          console.write_raw("\x04"); // Ctrl+D
          console.expect("false");

          // prompt returns null by pressing Ctrl+C
          console.write_line_raw("prompt()");
          console.expect("Prompt ");
          console.write_raw("\x03"); // Ctrl+C
          console.expect("null");

          // prompt returns null by pressing Ctrl+D
          console.write_line_raw("prompt()");
          console.expect("Prompt ");
          console.write_raw("\x04"); // Ctrl+D
          console.expect("null");
        }
      });
  }

  itest!(dynamic_import_static_analysis_no_permissions {
    args: "run --quiet --reload --no-prompt dynamic_import/static_analysis_no_permissions.ts",
    output: "dynamic_import/static_analysis_no_permissions.ts.out",
  });

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
  args: "run --quiet --reload --allow-net --allow-read --unstable --cert tls/RootCA.pem run/tls_starttls.js",
  output: "run/tls.out",
});

itest!(tls_connecttls {
  args: "run --quiet --reload --allow-net --allow-read --cert tls/RootCA.pem run/tls_connecttls.js",
  output: "run/tls.out",
});

itest!(byte_order_mark {
  args: "run --no-check run/byte_order_mark.ts",
  output: "run/byte_order_mark.out",
});

#[test]
fn issue9750() {
  TestContext::default()
    .new_command()
    .args_vec(["run", "--prompt", "run/issue9750.js"])
    .with_pty(|mut console| {
      console.expect("Enter 'yy':");
      console.write_line_raw("yy");
      console.expect(concat!(
        "┌ ⚠️  Deno requests env access.\r\n",
        "├ Requested by `Deno.permissions.request()` API.\r\n",
        "├ Run again with --allow-env to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.write_line_raw("n");
      console.expect("Denied env access.");
      console.expect(concat!(
        "┌ ⚠️  Deno requests env access to \"SECRET\".\r\n",
        "├ Run again with --allow-env to bypass this prompt.\r\n",
        "└ Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all env permissions)",
      ));
      console.write_line_raw("n");
      console.expect_all(&[
        "Denied env access to \"SECRET\".",
        "PermissionDenied: Requires env access to \"SECRET\", run again with the --allow-env flag",
      ]);
    });
}

// Regression test for https://github.com/denoland/deno/issues/11451.
itest!(dom_exception_formatting {
  args: "run run/dom_exception_formatting.ts",
  output: "run/dom_exception_formatting.ts.out",
  exit_code: 1,
});

itest!(long_data_url_formatting {
  args: "run run/long_data_url_formatting.ts",
  output: "run/long_data_url_formatting.ts.out",
  exit_code: 1,
});

itest!(eval_context_throw_dom_exception {
  args: "run run/eval_context_throw_dom_exception.js",
  output: "run/eval_context_throw_dom_exception.js.out",
});

#[test]
#[cfg(unix)]
fn navigator_language_unix() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_language.ts",
    None,
    Some(vec![("LC_ALL".to_owned(), "pl_PL".to_owned())]),
    false,
  );
  assert_eq!(res, "pl-PL\n")
}

#[test]
fn navigator_language() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_language.ts",
    None,
    None,
    false,
  );
  assert!(!res.is_empty())
}

#[test]
#[cfg(unix)]
fn navigator_languages_unix() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_languages.ts",
    None,
    Some(vec![
      ("LC_ALL".to_owned(), "pl_PL".to_owned()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert_eq!(res, "[ \"pl-PL\" ]\n")
}

#[test]
fn navigator_languages() {
  let (res, _) = util::run_and_collect_output(
    true,
    "run navigator_languages.ts",
    None,
    None,
    false,
  );
  assert!(!res.is_empty())
}

/// Regression test for https://github.com/denoland/deno/issues/12740.
#[test]
fn issue12740() {
  let mod_dir = TempDir::new();
  let mod1_path = mod_dir.path().join("mod1.ts");
  let mod2_path = mod_dir.path().join("mod2.ts");
  mod1_path.write("");
  let status = util::deno_cmd()
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
  mod1_path.write("export { foo } from \"./mod2.ts\";");
  mod2_path.write("(");
  let status = util::deno_cmd()
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
  // With a fresh `DENO_DIR`, run a module with a dependency and a type error.
  mod1_path.write("import './mod2.ts'; Deno.exit('0');");
  mod2_path.write("console.log('Hello, world!');");
  let status = util::deno_cmd()
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
  let status = util::deno_cmd()
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
  args: "run run/issue13562.ts",
  output: "run/issue13562.ts.out",
});

itest!(import_attributes_static_import {
  args: "run --allow-read import_attributes/static_import.ts",
  output: "import_attributes/static_import.out",
});

itest!(import_attributes_static_export {
  args: "run --allow-read import_attributes/static_export.ts",
  output: "import_attributes/static_export.out",
});

itest!(import_attributes_static_error {
  args: "run --allow-read import_attributes/static_error.ts",
  output: "import_attributes/static_error.out",
  exit_code: 1,
});

itest!(import_attributes_dynamic_import {
  args: "run --allow-read --check import_attributes/dynamic_import.ts",
  output: "import_attributes/dynamic_import.out",
});

itest!(import_attributes_dynamic_error {
  args: "run --allow-read import_attributes/dynamic_error.ts",
  output: "import_attributes/dynamic_error.out",
  exit_code: 1,
});

itest!(import_attributes_type_check {
  args: "run --allow-read --check import_attributes/type_check.ts",
  output: "import_attributes/type_check.out",
  exit_code: 1,
});

itest!(delete_window {
  args: "run run/delete_window.js",
  output_str: Some("true\n"),
});

itest!(colors_without_global_this {
  args: "run run/colors_without_globalThis.js",
  output_str: Some("true\n"),
});

itest!(config_auto_discovered_for_local_script {
  args: "run --quiet run/with_config/frontend_work.ts",
  output_str: Some("ok\n"),
});

itest!(config_auto_discovered_for_local_script_log {
  args: "run -L debug run/with_config/frontend_work.ts",
  output: "run/with_config/auto_discovery_log.out",
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

itest!(package_json_auto_discovered_for_local_script_arg {
  args: "run -L debug -A no_deno_json/main.ts",
  output: "run/with_package_json/no_deno_json/main.out",
  // notice this is not in no_deno_json
  cwd: Some("run/with_package_json/"),
  // prevent creating a node_modules dir in the code directory
  copy_temp_dir: Some("run/with_package_json/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

// In this case we shouldn't discover `package.json` file, because it's in a
// directory that is above the directory containing `deno.json` file.
itest!(
  package_json_auto_discovered_for_local_script_arg_with_stop {
    args: "run -L debug with_stop/some/nested/dir/main.ts",
    output: "run/with_package_json/with_stop/main.out",
    cwd: Some("run/with_package_json/"),
    copy_temp_dir: Some("run/with_package_json/"),
    envs: env_vars_for_npm_tests(),
    http_server: true,
    exit_code: 1,
  }
);

itest!(package_json_not_auto_discovered_no_config {
  args: "run -L debug -A --no-config noconfig.ts",
  output: "run/with_package_json/no_deno_json/noconfig.out",
  cwd: Some("run/with_package_json/no_deno_json/"),
});

itest!(package_json_not_auto_discovered_no_npm {
  args: "run -L debug -A --no-npm noconfig.ts",
  output: "run/with_package_json/no_deno_json/noconfig.out",
  cwd: Some("run/with_package_json/no_deno_json/"),
});

itest!(package_json_not_auto_discovered_env_var {
  args: "run -L debug -A noconfig.ts",
  output: "run/with_package_json/no_deno_json/noconfig.out",
  cwd: Some("run/with_package_json/no_deno_json/"),
  envs: vec![("DENO_NO_PACKAGE_JSON".to_string(), "1".to_string())],
});

itest!(
  package_json_auto_discovered_node_modules_relative_package_json {
    args: "run -A main.js",
    output: "run/with_package_json/no_deno_json/sub_dir/main.out",
    cwd: Some("run/with_package_json/no_deno_json/sub_dir"),
    copy_temp_dir: Some("run/with_package_json/no_deno_json/"),
    envs: env_vars_for_npm_tests(),
    http_server: true,
  }
);

itest!(package_json_auto_discovered_for_npm_binary {
  args: "run -L debug -A npm:@denotest/bin/cli-esm this is a test",
  output: "run/with_package_json/npm_binary/main.out",
  cwd: Some("run/with_package_json/npm_binary/"),
  copy_temp_dir: Some("run/with_package_json/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

itest!(package_json_auto_discovered_no_package_json_imports {
  // this should not use --quiet because we should ensure no package.json install occurs
  args: "run -A no_package_json_imports.ts",
  output: "run/with_package_json/no_deno_json/no_package_json_imports.out",
  cwd: Some("run/with_package_json/no_deno_json"),
  copy_temp_dir: Some("run/with_package_json/no_deno_json"),
});

#[test]
fn package_json_with_deno_json() {
  let context = TestContextBuilder::for_npm()
    .use_copy_temp_dir("package_json/deno_json/")
    .cwd("package_json/deno_json/")
    .build();
  let output = context.new_command().args("run --quiet -A main.ts").run();
  output.assert_matches_file("package_json/deno_json/main.out");

  assert!(context
    .temp_dir()
    .path()
    .join("package_json/deno_json/deno.lock")
    .exists());

  // run again and ensure the top level install doesn't happen twice
  let output = context
    .new_command()
    .args("run --log-level=debug -A main.ts")
    .run();
  let output = output.combined_output();
  assert_contains!(output, "Skipping top level install.");
}

#[test]
fn package_json_error_dep_value_test() {
  let context = TestContextBuilder::for_npm()
    .use_copy_temp_dir("package_json/invalid_value")
    .cwd("package_json/invalid_value")
    .build();

  // should run fine when not referencing a failing dep entry
  context
    .new_command()
    .args("run ok.ts")
    .run()
    .assert_matches_file("package_json/invalid_value/ok.ts.out");

  // should fail when referencing a failing dep entry
  context
    .new_command()
    .args("run error.ts")
    .run()
    .assert_exit_code(1)
    .assert_matches_file("package_json/invalid_value/error.ts.out");

  // should output a warning about the failing dep entry
  context
    .new_command()
    .args("task test")
    .run()
    .assert_matches_file("package_json/invalid_value/task.out");
}

#[test]
fn package_json_no_node_modules_dir_created() {
  // it should not create a node_modules directory
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  temp_dir.write("deno.json", "{}");
  temp_dir.write("package.json", "{}");
  temp_dir.write("main.ts", "");

  context.new_command().args("run main.ts").run();

  assert!(!temp_dir.path().join("node_modules").exists());
}

#[test]
fn node_modules_dir_no_npm_specifiers_no_dir_created() {
  // it should not create a node_modules directory
  let context = TestContextBuilder::new()
    .add_npm_env_vars()
    .use_temp_cwd()
    .build();
  let temp_dir = context.temp_dir();

  temp_dir.write("deno.json", "{}");
  temp_dir.write("main.ts", "");

  context
    .new_command()
    .args("run --node-modules-dir main.ts")
    .run();

  assert!(!temp_dir.path().join("node_modules").exists());
}

itest!(wasm_streaming_panic_test {
  args: "run run/wasm_streaming_panic_test.js",
  output: "run/wasm_streaming_panic_test.js.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/13897.
itest!(fetch_async_error_stack {
  args: "run --quiet -A run/fetch_async_error_stack.ts",
  output: "run/fetch_async_error_stack.ts.out",
  exit_code: 1,
});

itest!(unstable_ffi_1 {
  args: "run run/ffi/unstable_ffi_1.js",
  output: "run/ffi/unstable_ffi_1.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_2 {
  args: "run run/ffi/unstable_ffi_2.js",
  output: "run/ffi/unstable_ffi_2.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_3 {
  args: "run run/ffi/unstable_ffi_3.js",
  output: "run/ffi/unstable_ffi_3.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_4 {
  args: "run run/ffi/unstable_ffi_4.js",
  output: "run/ffi/unstable_ffi_4.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_5 {
  args: "run run/ffi/unstable_ffi_5.js",
  output: "run/ffi/unstable_ffi_5.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_6 {
  args: "run run/ffi/unstable_ffi_6.js",
  output: "run/ffi/unstable_ffi_6.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_7 {
  args: "run run/ffi/unstable_ffi_7.js",
  output: "run/ffi/unstable_ffi_7.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_8 {
  args: "run run/ffi/unstable_ffi_8.js",
  output: "run/ffi/unstable_ffi_8.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_9 {
  args: "run run/ffi/unstable_ffi_9.js",
  output: "run/ffi/unstable_ffi_9.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_10 {
  args: "run run/ffi/unstable_ffi_10.js",
  output: "run/ffi/unstable_ffi_10.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_11 {
  args: "run run/ffi/unstable_ffi_11.js",
  output: "run/ffi/unstable_ffi_11.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_12 {
  args: "run run/ffi/unstable_ffi_12.js",
  output: "run/ffi/unstable_ffi_12.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_13 {
  args: "run run/ffi/unstable_ffi_13.js",
  output: "run/ffi/unstable_ffi_13.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_14 {
  args: "run run/ffi/unstable_ffi_14.js",
  output: "run/ffi/unstable_ffi_14.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_15 {
  args: "run run/ffi/unstable_ffi_15.js",
  output: "run/ffi/unstable_ffi_15.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_16 {
  args: "run run/ffi/unstable_ffi_16.js",
  output: "run/ffi/unstable_ffi_16.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_17 {
  args: "run run/ffi/unstable_ffi_17.js",
  output: "run/ffi/unstable_ffi_17.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_18 {
  args: "run run/ffi/unstable_ffi_18.js",
  output: "run/ffi/unstable_ffi_18.js.out",
  exit_code: 70,
});

itest!(unstable_ffi_19 {
  args: "run run/ffi/unstable_ffi_19.js",
  output: "run/ffi/unstable_ffi_19.js.out",
  exit_code: 70,
});

itest!(event_listener_error {
  args: "run --quiet run/event_listener_error.ts",
  output: "run/event_listener_error.ts.out",
  exit_code: 1,
});

itest!(event_listener_error_handled {
  args: "run --quiet run/event_listener_error_handled.ts",
  output: "run/event_listener_error_handled.ts.out",
});

// https://github.com/denoland/deno/pull/14159#issuecomment-1092285446
itest!(event_listener_error_immediate_exit {
  args: "run --quiet run/event_listener_error_immediate_exit.ts",
  output: "run/event_listener_error_immediate_exit.ts.out",
  exit_code: 1,
});

// https://github.com/denoland/deno/pull/14159#issuecomment-1092285446
itest!(event_listener_error_immediate_exit_worker {
  args:
    "run --quiet --unstable -A run/event_listener_error_immediate_exit_worker.ts",
  output: "run/event_listener_error_immediate_exit_worker.ts.out",
  exit_code: 1,
});

itest!(set_timeout_error {
  args: "run --quiet run/set_timeout_error.ts",
  output: "run/set_timeout_error.ts.out",
  exit_code: 1,
});

itest!(set_timeout_error_handled {
  args: "run --quiet run/set_timeout_error_handled.ts",
  output: "run/set_timeout_error_handled.ts.out",
});

itest!(aggregate_error {
  args: "run --quiet run/aggregate_error.ts",
  output: "run/aggregate_error.out",
  exit_code: 1,
});

itest!(complex_error {
  args: "run --quiet run/complex_error.ts",
  output: "run/complex_error.ts.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/16340.
itest!(error_with_errors_prop {
  args: "run --quiet run/error_with_errors_prop.js",
  output: "run/error_with_errors_prop.js.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/12143.
itest!(js_root_with_ts_check {
  args: "run --quiet --check run/js_root_with_ts_check.js",
  output: "run/js_root_with_ts_check.js.out",
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
    .stderr_piped()
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
  args: "run --quiet --check --config run/checkjs.tsconfig.json run/check_js_points_to_ts/test.js",
  output: "run/check_js_points_to_ts/test.js.out",
  exit_code: 1,
});

itest!(no_prompt_flag {
  args: "run --quiet --unstable --no-prompt run/no_prompt.ts",
  output_str: Some(""),
});

#[test]
fn deno_no_prompt_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("run/no_prompt.ts")
    .env("DENO_NO_PROMPT", "1")
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

itest!(report_error {
  args: "run --quiet run/report_error.ts",
  output: "run/report_error.ts.out",
  exit_code: 1,
});

itest!(report_error_handled {
  args: "run --quiet run/report_error_handled.ts",
  output: "run/report_error_handled.ts.out",
});

// Regression test for https://github.com/denoland/deno/issues/15513.
itest!(report_error_end_of_program {
  args: "run --quiet run/report_error_end_of_program.ts",
  output: "run/report_error_end_of_program.ts.out",
  exit_code: 1,
});

itest!(queue_microtask_error {
  args: "run --quiet run/queue_microtask_error.ts",
  output: "run/queue_microtask_error.ts.out",
  exit_code: 1,
});

itest!(queue_microtask_error_handled {
  args: "run --quiet run/queue_microtask_error_handled.ts",
  output: "run/queue_microtask_error_handled.ts.out",
});

itest!(spawn_stdout_inherit {
  args: "run --quiet --unstable -A run/spawn_stdout_inherit.ts",
  output: "run/spawn_stdout_inherit.ts.out",
});

itest!(error_name_non_string {
  args: "run --quiet run/error_name_non_string.js",
  output: "run/error_name_non_string.js.out",
  exit_code: 1,
});

itest!(custom_inspect_url {
  args: "run run/custom_inspect_url.js",
  output: "run/custom_inspect_url.js.out",
});

itest!(config_json_import {
  args: "run --quiet -c jsx/deno-jsx.json run/config_json_import.ts",
  output: "run/config_json_import.ts.out",
  http_server: true,
});

#[test]
fn running_declaration_files() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let files = vec!["file.d.ts", "file.d.cts", "file.d.mts"];

  for file in files {
    temp_dir.write(file, "");
    context
      .new_command()
      .args_vec(["run", file])
      .run()
      .skip_output_check()
      .assert_exit_code(0);
  }
}

itest!(test_and_bench_are_noops_in_run {
  args: "run run/test_and_bench_in_run.js",
  output_str: Some(""),
});

#[cfg(not(target_os = "windows"))]
itest!(spawn_kill_permissions {
  args: "run --quiet --unstable --allow-run=cat spawn_kill_permissions.ts",
  output_str: Some(""),
});

itest!(followup_dyn_import_resolved {
  args: "run --unstable --allow-read run/followup_dyn_import_resolves/main.ts",
  output: "run/followup_dyn_import_resolves/main.ts.out",
});

itest!(allow_run_allowlist_resolution {
  args: "run --quiet --unstable -A allow_run_allowlist_resolution.ts",
  output: "allow_run_allowlist_resolution.ts.out",
});

itest!(unhandled_rejection {
  args: "run --check run/unhandled_rejection.ts",
  output: "run/unhandled_rejection.ts.out",
});

itest!(unhandled_rejection_sync_error {
  args: "run --check run/unhandled_rejection_sync_error.ts",
  output: "run/unhandled_rejection_sync_error.ts.out",
});

// Regression test for https://github.com/denoland/deno/issues/15661
itest!(unhandled_rejection_dynamic_import {
  args: "run --allow-read run/unhandled_rejection_dynamic_import/main.ts",
  output: "run/unhandled_rejection_dynamic_import/main.ts.out",
  exit_code: 1,
});

// Regression test for https://github.com/denoland/deno/issues/16909
itest!(unhandled_rejection_dynamic_import2 {
  args: "run --allow-read run/unhandled_rejection_dynamic_import2/main.ts",
  output: "run/unhandled_rejection_dynamic_import2/main.ts.out",
});

itest!(nested_error {
  args: "run run/nested_error/main.ts",
  output: "run/nested_error/main.ts.out",
  exit_code: 1,
});

itest!(node_env_var_allowlist {
  args: "run --unstable --no-prompt run/node_env_var_allowlist.ts",
  output: "run/node_env_var_allowlist.ts.out",
  exit_code: 1,
});

#[test]
fn cache_test() {
  let _g = util::http_server();
  let deno_dir = TempDir::new();
  let module_url =
    url::Url::parse("http://localhost:4545/run/006_url_imports.ts").unwrap();
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg("--check=all")
    .arg("-L")
    .arg("debug")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());

  let prg = util::deno_exe_path();
  let output = Command::new(prg)
    .env("DENO_DIR", deno_dir.path())
    .env("HTTP_PROXY", "http://nil")
    .env("NO_COLOR", "1")
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");

  let str_output = std::str::from_utf8(&output.stdout).unwrap();

  let module_output_path =
    util::testdata_path().join("run/006_url_imports.ts.out");
  let mut module_output = String::new();
  let mut module_output_file = std::fs::File::open(module_output_path).unwrap();
  module_output_file
    .read_to_string(&mut module_output)
    .unwrap();

  assert_eq!(module_output, str_output);
}

#[test]
fn cache_invalidation_test() {
  let deno_dir = TempDir::new();
  let fixture_path = deno_dir.path().join("fixture.ts");
  fixture_path.write("console.log(\"42\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(&fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  fixture_path.write("console.log(\"43\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg(fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn cache_invalidation_test_no_check() {
  let deno_dir = TempDir::new();
  let fixture_path = deno_dir.path().join("fixture.ts");
  fixture_path.write("console.log(\"42\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(&fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "42\n");
  fixture_path.write("console.log(\"43\");");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--no-check")
    .arg(fixture_path)
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let actual = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(actual, "43\n");
}

#[test]
fn ts_dependency_recompilation() {
  let t = TempDir::new();
  let ats = t.path().join("a.ts");

  std::fs::write(
    &ats,
    "
    import { foo } from \"./b.ts\";

    function print(str: string): void {
        console.log(str);
    }

    print(foo);",
  )
  .unwrap();

  let bts = t.path().join("b.ts");
  std::fs::write(
    &bts,
    "
    export const foo = \"foo\";",
  )
  .unwrap();

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--check")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  assert!(stdout_output.ends_with("foo"));
  assert!(stderr_output.starts_with("Check"));

  // Overwrite contents of b.ts and run again
  std::fs::write(
    &bts,
    "
    export const foo = 5;",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg("--check")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  // error: TS2345 [ERROR]: Argument of type '5' is not assignable to parameter of type 'string'.
  assert!(stderr_output.contains("TS2345"));
  assert!(!output.status.success());
  assert!(stdout_output.is_empty());
}

#[test]
fn basic_auth_tokens() {
  let _g = util::http_server();

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://127.0.0.1:4554/run/001_hello.js")
    .piped_output()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout_str.is_empty());

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{stderr_str}");

  assert!(stderr_str
    .contains("Module not found \"http://127.0.0.1:4554/run/001_hello.js\"."));

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://127.0.0.1:4554/run/001_hello.js")
    .env("DENO_AUTH_TOKENS", "testuser123:testpassabc@127.0.0.1:4554")
    .piped_output()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  eprintln!("{stderr_str}");

  assert!(output.status.success());

  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!(util::strip_ansi_codes(stdout_str), "Hello World");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_resolve_dns() {
  use std::net::SocketAddr;
  use std::str::FromStr;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::net::TcpListener;
  use tokio::net::UdpSocket;
  use tokio::sync::oneshot;
  use trust_dns_server::authority::Catalog;
  use trust_dns_server::authority::ZoneType;
  use trust_dns_server::proto::rr::Name;
  use trust_dns_server::store::in_memory::InMemoryAuthority;
  use trust_dns_server::ServerFuture;

  const DNS_PORT: u16 = 4553;

  // Setup DNS server for testing
  async fn run_dns_server(tx: oneshot::Sender<()>) {
    let zone_file = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.zone.in"),
    )
    .unwrap();
    let lexer = Lexer::new(&zone_file);
    let records = Parser::new().parse(
      lexer,
      Some(Name::from_str("example.com").unwrap()),
      None,
    );
    if records.is_err() {
      panic!("failed to parse: {:?}", records.err())
    }
    let (origin, records) = records.unwrap();
    let authority = Box::new(Arc::new(
      InMemoryAuthority::new(origin, records, ZoneType::Primary, false)
        .unwrap(),
    ));
    let mut catalog: Catalog = Catalog::new();
    catalog.upsert(Name::root().into(), authority);

    let mut server_fut = ServerFuture::new(catalog);
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], DNS_PORT));
    let tcp_listener = TcpListener::bind(socket_addr).await.unwrap();
    let udp_socket = UdpSocket::bind(socket_addr).await.unwrap();
    server_fut.register_socket(udp_socket);
    server_fut.register_listener(tcp_listener, Duration::from_secs(2));

    // Notifies that the DNS server is ready
    tx.send(()).unwrap();

    server_fut.block_until_done().await.unwrap();
  }

  let (ready_tx, ready_rx) = oneshot::channel();
  let dns_server_fut = run_dns_server(ready_tx);
  let handle = tokio::spawn(dns_server_fut);

  // Waits for the DNS server to be ready
  ready_rx.await.unwrap();

  // Pass: `--allow-net`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    println!("{err}");
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.ts.out"),
    )
    .unwrap();
    assert_eq!(expected, out);
  }

  // Pass: `--allow-net=127.0.0.1:4553`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net=127.0.0.1:4553")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
      eprintln!("stderr: {err}");
    }
    assert!(output.status.success());
    assert!(err.starts_with("Check file"));

    let expected = std::fs::read_to_string(
      util::testdata_path().join("run/resolve_dns.ts.out"),
    )
    .unwrap();
    assert_eq!(expected, out);
  }

  // Permission error: `--allow-net=deno.land`
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("--allow-net=deno.land")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) PermissionDenied: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  // Permission error: no permission specified
  {
    let output = util::deno_cmd()
      .current_dir(util::testdata_path())
      .env("NO_COLOR", "1")
      .arg("run")
      .arg("--check")
      .arg("run/resolve_dns.ts")
      .piped_output()
      .spawn()
      .unwrap()
      .wait_with_output()
      .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(err.starts_with("Check file"));
    assert!(err.contains(r#"error: Uncaught (in promise) PermissionDenied: Requires net access to "127.0.0.1:4553""#));
    assert!(out.is_empty());
  }

  handle.abort();
}

#[tokio::test]
async fn http2_request_url() {
  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--quiet")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("./run/http2_request_url.ts")
    .arg("4506")
    .stdout_piped()
    .spawn()
    .unwrap();
  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let cert = reqwest::Certificate::from_pem(include_bytes!(
    "../testdata/tls/RootCA.crt"
  ))
  .unwrap();

  let client = reqwest::Client::builder()
    .add_root_certificate(cert)
    .http2_prior_knowledge()
    .build()
    .unwrap();

  let res = client.get("http://127.0.0.1:4506").send().await.unwrap();
  assert_eq!(200, res.status());

  let body = res.text().await.unwrap();
  assert_eq!(body, "http://127.0.0.1:4506/");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[cfg(not(windows))]
#[test]
fn set_raw_should_not_panic_on_no_tty() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("Deno.stdin.setRaw(true)")
    // stdin set to piped so it certainly does not refer to TTY
    .stdin(std::process::Stdio::piped())
    // stderr is piped so we can capture output.
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert!(stderr.contains("BadResource"));
}

#[test]
fn timeout_clear() {
  // https://github.com/denoland/deno/issues/7599

  use std::time::Duration;
  use std::time::Instant;

  let source_code = r#"
const handle = setTimeout(() => {
  console.log("timeout finish");
}, 10000);
clearTimeout(handle);
console.log("finish");
"#;

  let mut p = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let stdin = p.stdin.as_mut().unwrap();
  stdin.write_all(source_code.as_bytes()).unwrap();
  let start = Instant::now();
  let status = p.wait().unwrap();
  let end = Instant::now();
  assert!(status.success());
  // check that program did not run for 10 seconds
  // for timeout to clear
  assert!(end - start < Duration::new(10, 0));
}

#[test]
fn broken_stdout() {
  let (reader, writer) = os_pipe::pipe().unwrap();
  // drop the reader to create a broken pipe
  drop(reader);

  let output = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("eval")
    .arg("console.log(3.14)")
    .stdout(writer)
    .stderr_piped()
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();

  assert!(!output.status.success());
  let stderr = std::str::from_utf8(output.stderr.as_ref()).unwrap().trim();
  assert!(stderr.contains("Uncaught (in promise) BrokenPipe"));
  assert!(!stderr.contains("panic"));
}

itest!(error_cause {
  args: "run run/error_cause.ts",
  output: "run/error_cause.ts.out",
  exit_code: 1,
});

itest!(error_cause_recursive_aggregate {
  args: "run error_cause_recursive_aggregate.ts",
  output: "error_cause_recursive_aggregate.ts.out",
  exit_code: 1,
});

itest!(error_cause_recursive_tail {
  args: "run error_cause_recursive_tail.ts",
  output: "error_cause_recursive_tail.ts.out",
  exit_code: 1,
});

itest!(error_cause_recursive {
  args: "run run/error_cause_recursive.ts",
  output: "run/error_cause_recursive.ts.out",
  exit_code: 1,
});

itest!(default_file_extension_is_js {
  args: "run --check file_extensions/js_without_extension",
  output: "file_extensions/js_without_extension.out",
  exit_code: 0,
});

itest!(js_without_extension {
  args: "run --ext js --check file_extensions/js_without_extension",
  output: "file_extensions/js_without_extension.out",
  exit_code: 0,
});

itest!(ts_without_extension {
  args: "run --ext ts --check file_extensions/ts_without_extension",
  output: "file_extensions/ts_without_extension.out",
  exit_code: 0,
});

itest!(ext_flag_takes_precedence_over_extension {
  args: "run --ext ts --check file_extensions/ts_with_js_extension.js",
  output: "file_extensions/ts_with_js_extension.out",
  exit_code: 0,
});

#[test]
fn websocket() {
  let _g = util::http_server();

  let script = util::testdata_path().join("run/websocket_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let status = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .spawn()
    .unwrap()
    .wait()
    .unwrap();

  assert!(status.success());
}

#[ignore]
#[test]
fn websocketstream() {
  let _g = util::http_server();

  let script = util::testdata_path().join("run/websocketstream_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let status = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .spawn()
    .unwrap()
    .wait()
    .unwrap();

  assert!(status.success());
}

#[tokio::test(flavor = "multi_thread")]
async fn websocketstream_ping() {
  let _g = util::http_server();

  let script = util::testdata_path().join("run/websocketstream_ping_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");

  let srv_fn = hyper::service::service_fn(|mut req| async move {
    let (response, upgrade_fut) =
      fastwebsockets::upgrade::upgrade(&mut req).unwrap();
    tokio::spawn(async move {
      let mut ws = upgrade_fut.await.unwrap();

      ws.write_frame(fastwebsockets::Frame::text(b"A"[..].into()))
        .await
        .unwrap();
      ws.write_frame(fastwebsockets::Frame::new(
        true,
        fastwebsockets::OpCode::Ping,
        None,
        vec![].into(),
      ))
      .await
      .unwrap();
      ws.write_frame(fastwebsockets::Frame::text(b"B"[..].into()))
        .await
        .unwrap();
      let message = ws.read_frame().await.unwrap();
      assert_eq!(message.opcode, fastwebsockets::OpCode::Pong);
      ws.write_frame(fastwebsockets::Frame::text(b"C"[..].into()))
        .await
        .unwrap();
      ws.write_frame(fastwebsockets::Frame::close_raw(vec![].into()))
        .await
        .unwrap();
    });
    Ok::<_, std::convert::Infallible>(response)
  });

  let child = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();
  let server = tokio::net::TcpListener::bind("127.0.0.1:4513")
    .await
    .unwrap();
  tokio::spawn(async move {
    let (stream, _) = server.accept().await.unwrap();
    let io = hyper_util::rt::TokioIo::new(stream);
    let conn_fut = hyper::server::conn::http1::Builder::new()
      .serve_connection(io, srv_fn)
      .with_upgrades();

    if let Err(e) = conn_fut.await {
      eprintln!("websocket server error: {e:?}");
    }
  });

  let r = child.wait_with_output().unwrap();
  assert!(r.status.success());
}

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
  Fut: std::future::Future + Send + 'static,
  Fut::Output: Send + 'static,
{
  fn execute(&self, fut: Fut) {
    deno_core::unsync::spawn(fut);
  }
}

#[tokio::test]
async fn websocket_server_multi_field_connection_header() {
  let script = util::testdata_path()
    .join("run/websocket_server_multi_field_connection_header_test.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let stream = tokio::net::TcpStream::connect("localhost:4319")
    .await
    .unwrap();
  let req = http::Request::builder()
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "keep-alive, Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    )
    .header("Sec-WebSocket-Version", "13")
    .uri("ws://localhost:4319")
    .body(http_body_util::Empty::<Bytes>::new())
    .unwrap();

  let (mut socket, _) =
    fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
      .await
      .unwrap();

  let message = socket.read_frame().await.unwrap();
  assert_eq!(message.opcode, fastwebsockets::OpCode::Close);
  assert!(message.payload.is_empty());
  socket
    .write_frame(fastwebsockets::Frame::close_raw(vec![].into()))
    .await
    .unwrap();
  assert!(child.wait().unwrap().success());
}

// TODO(bartlomieju): this should use `deno run`, not `deno test`; but the
// test hangs then. https://github.com/denoland/deno/issues/14283
#[tokio::test]
async fn websocket_server_idletimeout() {
  let script =
    util::testdata_path().join("run/websocket_server_idletimeout.ts");
  let root_ca = util::testdata_path().join("tls/RootCA.pem");
  let mut child = util::deno_cmd()
    .arg("test")
    .arg("--unstable")
    .arg("--allow-net")
    .arg("--cert")
    .arg(root_ca)
    .arg(script)
    .stdout_piped()
    .spawn()
    .unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut buffer = [0; 5];
  let read = stdout.read(&mut buffer).unwrap();
  assert_eq!(read, 5);
  let msg = std::str::from_utf8(&buffer).unwrap();
  assert_eq!(msg, "READY");

  let stream = tokio::net::TcpStream::connect("localhost:4509")
    .await
    .unwrap();
  let req = http::Request::builder()
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "keep-alive, Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    )
    .header("Sec-WebSocket-Version", "13")
    .uri("ws://localhost:4509")
    .body(http_body_util::Empty::<Bytes>::new())
    .unwrap();

  let (_socket, _) =
    fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
      .await
      .unwrap();

  assert!(child.wait().unwrap().success());
}

itest!(auto_discover_lockfile {
  args: "run run/auto_discover_lockfile/main.ts",
  output: "run/auto_discover_lockfile/main.out",
  http_server: true,
  exit_code: 10,
});

itest!(no_lock_flag {
  args: "run --no-lock run/no_lock_flag/main.ts",
  output: "run/no_lock_flag/main.out",
  http_server: true,
  exit_code: 0,
});

itest!(config_file_lock_false {
  args: "run --config=run/config_file_lock_boolean/false.json run/config_file_lock_boolean/main.ts",
  output: "run/config_file_lock_boolean/false.main.out",
  http_server: true,
  exit_code: 0,
});

itest!(config_file_lock_true {
  args: "run --config=run/config_file_lock_boolean/true.json run/config_file_lock_boolean/main.ts",
  output: "run/config_file_lock_boolean/true.main.out",
  http_server: true,
  exit_code: 10,
});

itest!(permission_args {
  args: "run run/001_hello.js --allow-net",
  output: "run/permission_args.out",
  envs: vec![("NO_COLOR".to_string(), "1".to_string())],
});

itest!(permission_args_quiet {
  args: "run --quiet run/001_hello.js --allow-net",
  output: "run/001_hello.js.out",
});

// Regression test for https://github.com/denoland/deno/issues/16772
#[test]
fn file_fetcher_preserves_permissions() {
  let context = TestContext::with_http_server();
  context
    .new_command()
    .args("repl --quiet")
    .with_pty(|mut console| {
      console.write_line(
      "const a = await import('http://localhost:4545/run/019_media_types.ts');",
    );
      console.expect("Allow?");
      console.write_line_raw("y");
      console.expect_all(&["success", "true"]);
    });
}

#[test]
fn stdio_streams_are_locked_in_permission_prompt() {
  if !util::pty::Pty::is_supported() {
    // Don't deal with the logic below if the with_pty
    // block doesn't even run (ex. on Windows CI)
    return;
  }

  let context = TestContextBuilder::new()
    .use_http_server()
    .use_copy_temp_dir("run/stdio_streams_are_locked_in_permission_prompt")
    .build();
  let mut passed_test = false;
  let mut i = 0;
  while !passed_test {
    i += 1;
    if i > 5 {
      panic!("Output happened before permission prompt too many times");
    }

    context
      .new_command()
      .args("repl --allow-read")
      .with_pty(|mut console| {
        let malicious_output = r#"Are you sure you want to continue?"#;

        console.write_line(r#"const url = "file://" + Deno.cwd().replace("\\", "/") + "/run/stdio_streams_are_locked_in_permission_prompt/worker.js";"#);
        console.expect("undefined");
        // ensure this file exists
        console.write_line(r#"const _file = Deno.readTextFileSync("./run/stdio_streams_are_locked_in_permission_prompt/worker.js");"#);
        console.expect("undefined");
        console.write_line(r#"new Worker(url, { type: "module" }); await Deno.writeTextFile("./text.txt", "some code");"#);
        console.expect("Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all write permissions)");

        // Due to the main thread being slow, it may occur that the worker thread outputs
        // before the permission prompt is shown. This is not a bug and just a timing issue
        // when dealing with multiple threads. If this occurs, detect such a case and then
        // retry running the test.
        if let Some(malicious_index) = console.all_output().find(malicious_output) {
          let prompt_index = console.all_output().find("Allow?").unwrap();
          // Ensure the malicious output is shown before the prompt as we
          // expect in this scenario. If not, that would indicate a bug.
          assert!(malicious_index < prompt_index);
          return;
        }

        std::thread::sleep(Duration::from_millis(50)); // give the other thread some time to output
        console.write_line_raw("invalid");
        console.expect("Unrecognized option.");
        console.expect("Allow? [y/n/A] (y = yes, allow; n = no, deny; A = allow all write permissions)");
        console.write_line_raw("y");
        console.expect("Granted write access to");

        // this output should now be shown below and not above
        console.expect(malicious_output);
        passed_test = true;
      });
  }
}

#[test]
fn permission_prompt_strips_ansi_codes_and_control_chars() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(
        r#"Deno.permissions.request({ name: "env", variable: "\rDo you like ice cream? y/n" });"#
      );
    // will be uppercase on windows
    let env_name = if cfg!(windows) {
      "DO YOU LIKE ICE CREAM? Y/N"
    } else {
      "Do you like ice cream? y/n"
    };
    console.expect(format!(
      "┌ ⚠️  Deno requests env access to \"{}\".",
      env_name
    ))
  });

  util::with_pty(&["repl"], |mut console| {
    console.write_line_raw(r#"const boldANSI = "\u001b[1m";"#);
    console.expect("undefined");
    console.write_line_raw(r#"const unboldANSI = "\u001b[22m";"#);
    console.expect("undefined");
    console.write_line_raw(r#"const prompt = `┌ ⚠️  ${boldANSI}Deno requests run access to "echo"${unboldANSI}\n ├ Requested by \`Deno.Command().output()`"#);
    console.expect("undefined");
    console.write_line_raw(r#"const moveANSIUp = "\u001b[1A";"#);
    console.expect("undefined");
    console.write_line_raw(r#"const clearANSI = "\u001b[2K";"#);
    console.expect("undefined");
    console.write_line_raw(r#"const moveANSIStart = "\u001b[1000D";"#);
    console.expect("undefined");

    console.write_line_raw(
      r#"Deno[Deno.internal].core.ops.op_spawn_child({
    cmd: "cat",
    args: ["file.txt"],
    clearEnv: false,
    cwd: undefined,
    env: [],
    uid: undefined,
    gid: undefined,
    stdin: "null",
    stdout: "inherit",
    stderr: "piped",
    signal: undefined,
    windowsRawArguments: false,
}, moveANSIUp + clearANSI + moveANSIStart + prompt)"#,
    );

    console.expect(r#"┌ ⚠️  Deno requests run access to "cat""#);
  });
}

itest!(node_builtin_modules_ts {
  args: "run --quiet --allow-read run/node_builtin_modules/mod.ts hello there",
  output: "run/node_builtin_modules/mod.ts.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
});

itest!(node_builtin_modules_js {
  args: "run --quiet --allow-read run/node_builtin_modules/mod.js hello there",
  output: "run/node_builtin_modules/mod.js.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
});

itest!(node_prefix_missing {
  args: "run --quiet run/node_prefix_missing/main.ts",
  output: "run/node_prefix_missing/main.ts.out",
  envs: env_vars_for_npm_tests(),
  exit_code: 1,
});

itest!(node_prefix_missing_unstable_bare_node_builtins_enbaled {
  args: "run --unstable-bare-node-builtins run/node_prefix_missing/main.ts",
  output: "run/node_prefix_missing/main.ts.out_feature_enabled",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
});

itest!(
  node_prefix_missing_unstable_bare_node_builtins_enbaled_by_env {
    args: "run run/node_prefix_missing/main.ts",
    output: "run/node_prefix_missing/main.ts.out_feature_enabled",
    envs: [
      env_vars_for_npm_tests(),
      vec![(
        "DENO_UNSTABLE_BARE_NODE_BUILTINS".to_string(),
        "1".to_string()
      )]
    ]
    .concat(),
    exit_code: 0,
  }
);

itest!(node_prefix_missing_unstable_bare_node_builtins_enbaled_by_config {
  args: "run --config=run/node_prefix_missing/config.json run/node_prefix_missing/main.ts",
  output: "run/node_prefix_missing/main.ts.out_feature_enabled",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
});

itest!(node_prefix_missing_unstable_bare_node_builtins_enbaled_with_import_map {
  args: "run --unstable-bare-node-builtins --import-map run/node_prefix_missing/import_map.json run/node_prefix_missing/main.ts",
  output: "run/node_prefix_missing/main.ts.out_feature_enabled",
  envs: env_vars_for_npm_tests(),
  exit_code: 0,
});

itest!(dynamic_import_syntax_error {
  args: "run -A run/dynamic_import_syntax_error.js",
  output: "run/dynamic_import_syntax_error.js.out",
  exit_code: 1,
});

itest!(extension_import {
  args: "run run/extension_import.ts",
  output: "run/extension_import.ts.out",
  exit_code: 1,
});

itest!(extension_dynamic_import {
  args: "run run/extension_dynamic_import.ts",
  output: "run/extension_dynamic_import.ts.out",
  exit_code: 1,
});

#[test]
pub fn vendor_dir_config_file() {
  let test_context = TestContextBuilder::new()
    .use_http_server()
    .use_temp_cwd()
    .build();
  let temp_dir = test_context.temp_dir();
  let vendor_dir = temp_dir.path().join("vendor");
  let rm_vendor_dir = || std::fs::remove_dir_all(&vendor_dir).unwrap();

  temp_dir.write("deno.json", r#"{ "vendor": true }"#);
  temp_dir.write(
    "main.ts",
    r#"import { returnsHi } from 'http://localhost:4545/subdir/mod1.ts';
console.log(returnsHi());"#,
  );

  let deno_run_cmd = test_context.new_command().args("run --quiet main.ts");
  deno_run_cmd.run().assert_matches_text("Hi\n");

  assert!(vendor_dir.exists());
  rm_vendor_dir();
  temp_dir.write("deno.json", r#"{ "vendor": false }"#);

  deno_run_cmd.run().assert_matches_text("Hi\n");
  assert!(!vendor_dir.exists());
  test_context
    .new_command()
    .args("cache --quiet --vendor main.ts")
    .run();
  assert!(vendor_dir.exists());
  rm_vendor_dir();

  temp_dir.write("deno.json", r#"{ "vendor": true }"#);
  let cache_command = test_context.new_command().args("cache --quiet main.ts");
  cache_command.run();

  assert!(vendor_dir.exists());
  let mod1_file = vendor_dir
    .join("http_localhost_4545")
    .join("subdir")
    .join("mod1.ts");
  mod1_file.write("export function returnsHi() { return 'bye bye bye'; }");

  // won't match the lockfile now
  deno_run_cmd
    .run()
    .assert_matches_text(r#"error: The source code is invalid, as it does not match the expected hash in the lock file.
  Specifier: http://localhost:4545/subdir/mod1.ts
  Lock file: [WILDCARD]deno.lock
"#)
    .assert_exit_code(10);

  // try updating by deleting the lockfile
  let lockfile = temp_dir.path().join("deno.lock");
  lockfile.remove_file();
  cache_command.run();

  // now it should run
  deno_run_cmd.run().assert_matches_text("bye bye bye\n");
  assert!(lockfile.exists());

  // ensure we can add and execute files in directories that have a hash in them
  test_context
    .new_command()
    // http_localhost_4545/subdir/#capitals_c75d7/main.js
    .args("cache http://localhost:4545/subdir/CAPITALS/main.js")
    .run()
    .skip_output_check();
  assert_eq!(
    vendor_dir.join("manifest.json").read_json_value(),
    json!({
      "folders": {
        "http://localhost:4545/subdir/CAPITALS/": "http_localhost_4545/subdir/#capitals_c75d7"
      }
    })
  );
  vendor_dir
    .join("http_localhost_4545/subdir/#capitals_c75d7/hello_there.ts")
    .write("console.log('hello there');");
  test_context
    .new_command()
    // todo(dsherret): seems wrong that we don't auto-discover the config file to get the vendor directory for this
    .args("run --vendor http://localhost:4545/subdir/CAPITALS/hello_there.ts")
    .run()
    .assert_matches_text("hello there\n");

  // now try importing directly from the vendor folder
  temp_dir.write(
    "main.ts",
    r#"import { returnsHi } from './vendor/http_localhost_4545/subdir/mod1.ts';
console.log(returnsHi());"#,
  );
  deno_run_cmd
    .run()
    .assert_matches_text("error: Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.
    at [WILDCARD]/main.ts:1:27
")
    .assert_exit_code(1);
}

itest!(explicit_resource_management {
  args: "run --quiet --check run/explicit_resource_management/main.ts",
  output: "run/explicit_resource_management/main.out",
});

itest!(workspaces_basic {
  args: "run -L debug -A --unstable-workspaces main.ts",
  output: "run/workspaces/basic/main.out",
  cwd: Some("run/workspaces/basic/"),
  copy_temp_dir: Some("run/workspaces/basic/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

itest!(workspaces_member_outside_root_dir {
  args: "run -A --unstable-workspaces main.ts",
  output: "run/workspaces/member_outside_root_dir/main.out",
  cwd: Some("run/workspaces/member_outside_root_dir/"),
  copy_temp_dir: Some("run/workspaces/member_outside_root_dir/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 1,
});

itest!(workspaces_nested_member {
  args: "run -A --unstable-workspaces main.ts",
  output: "run/workspaces/nested_member/main.out",
  cwd: Some("run/workspaces/nested_member/"),
  copy_temp_dir: Some("run/workspaces/nested_member/"),
  envs: env_vars_for_npm_tests(),
  http_server: true,
  exit_code: 1,
});

itest!(unsafe_proto {
  args: "run -A run/unsafe_proto/main.js",
  output: "run/unsafe_proto/main.out",
  http_server: false,
  exit_code: 0,
});

itest!(unsafe_proto_flag {
  args: "run -A --unstable-unsafe-proto run/unsafe_proto/main.js",
  output: "run/unsafe_proto/main_with_unsafe_proto_flag.out",
  http_server: false,
  exit_code: 0,
});

#[test]
fn test_unstable_sloppy_imports() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("a.ts", "export class A {}");
  temp_dir.write("b.js", "export class B {}");
  temp_dir.write("c.mts", "export class C {}");
  temp_dir.write("d.mjs", "export class D {}");
  temp_dir.write("e.tsx", "export class E {}");
  temp_dir.write("f.jsx", "export class F {}");
  let dir = temp_dir.path().join("dir");
  dir.create_dir_all();
  dir.join("index.tsx").write("export class G {}");
  temp_dir.write(
    "main.ts",
    r#"import * as a from "./a.js";
import * as b from "./b";
import * as c from "./c";
import * as d from "./d";
import * as e from "./e";
import * as e2 from "./e.js";
import * as f from "./f";
import * as g from "./dir";
console.log(a.A);
console.log(b.B);
console.log(c.C);
console.log(d.D);
console.log(e.E);
console.log(e2.E);
console.log(f.F);
console.log(g.G);
"#,
  );

  // run without sloppy imports
  context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(r#"error: Module not found "file:///[WILDCARD]/a.js". Maybe change the extension to '.ts' or run with --unstable-sloppy-imports
    at file:///[WILDCARD]/main.ts:1:20
"#)
    .assert_exit_code(1);

  // now run with sloppy imports
  temp_dir.write("deno.json", r#"{ "unstable": ["sloppy-imports"] }"#);
  context
    .new_command()
    .args("run main.ts")
    .run()
    .assert_matches_text(
      "Warning Sloppy imports are not recommended and have a negative impact on performance.
Warning Sloppy module resolution (hint: update .js extension to .ts)
    at file:///[WILDCARD]/main.ts:1:20
Warning Sloppy module resolution (hint: add .js extension)
    at file:///[WILDCARD]/main.ts:2:20
Warning Sloppy module resolution (hint: add .mts extension)
    at file:///[WILDCARD]/main.ts:3:20
Warning Sloppy module resolution (hint: add .mjs extension)
    at file:///[WILDCARD]/main.ts:4:20
Warning Sloppy module resolution (hint: add .tsx extension)
    at file:///[WILDCARD]/main.ts:5:20
Warning Sloppy module resolution (hint: update .js extension to .tsx)
    at file:///[WILDCARD]/main.ts:6:21
Warning Sloppy module resolution (hint: add .jsx extension)
    at file:///[WILDCARD]/main.ts:7:20
Warning Sloppy module resolution (hint: specify path to index.tsx file in directory instead)
    at file:///[WILDCARD]/main.ts:8:20
[class A]
[class B]
[class C]
[class D]
[class E]
[class E]
[class F]
[class G]
",
    );
}
