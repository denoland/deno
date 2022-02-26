// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::itest;
use tempfile::TempDir;
use test_util as util;

#[test]
fn info_with_compiled_source() {
  let _g = util::http_server();
  let module_path = "http://127.0.0.1:4545/048_media_types_jsx.ts";
  let t = TempDir::new().expect("tempdir fail");

  let mut deno = util::deno_cmd()
    .env("DENO_DIR", t.path())
    .current_dir(util::testdata_path())
    .arg("cache")
    .arg(&module_path)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());

  let output = util::deno_cmd()
    .env("DENO_DIR", t.path())
    .env("NO_COLOR", "1")
    .current_dir(util::testdata_path())
    .arg("info")
    .arg(&module_path)
    .output()
    .expect("failed to spawn script");

  let str_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  eprintln!("{}", str_output);
  // check the output of the test.ts program.
  assert!(str_output.contains("emit: "));
  assert_eq!(output.stderr, b"");
}

itest!(_022_info_flag_script {
  args: "info http://127.0.0.1:4545/019_media_types.ts",
  output: "022_info_flag_script.out",
  http_server: true,
});

itest!(_031_info_ts_error {
  args: "info 031_info_ts_error.ts",
  output: "031_info_ts_error.out",
});

itest!(_041_info_flag {
  args: "info",
  output: "041_info_flag.out",
});

itest!(_042_info_flag_location {
  args: "info --location https://deno.land",
  output: "041_info_flag_location.out",
});

itest!(info_json {
  args: "info --json --unstable",
  output: "info_json.out",
});

itest!(info_json_location {
  args: "info --json --unstable --location https://deno.land",
  output: "info_json_location.out",
});

itest!(_049_info_flag_script_jsx {
  args: "info http://127.0.0.1:4545/048_media_types_jsx.ts",
  output: "049_info_flag_script_jsx.out",
  http_server: true,
});

itest!(_055_info_file_json {
  args: "info --quiet --json --unstable 005_more_imports.ts",
  output: "055_info_file_json.out",
  exit_code: 0,
});

itest!(_065_import_map_info {
  args:
    "info --quiet --import-map=import_maps/import_map.json import_maps/test.ts",
  output: "065_import_map_info.out",
});

itest!(_076_info_json_deps_order {
  args: "info --unstable --json 076_info_json_deps_order.ts",
  output: "076_info_json_deps_order.out",
});

itest!(info_missing_module {
  args: "info error_009_missing_js_module.js",
  output: "info_missing_module.out",
});

itest!(info_recursive_modules {
  args: "info --quiet info_recursive_imports_test.ts",
  output: "info_recursive_imports_test.out",
  exit_code: 0,
});

itest!(info_type_import {
  args: "info info_type_import.ts",
  output: "info_type_import.out",
});

itest!(_054_info_local_imports {
  args: "info --quiet 005_more_imports.ts",
  output: "054_info_local_imports.out",
  exit_code: 0,
});

// Tests for AssertionError where "data" is unexpectedly null when
// a file contains only triple slash references (#11196)
itest!(data_null_error {
  args: "info info/data_null_error/mod.ts",
  output: "info/data_null_error/data_null_error.out",
});

itest!(deno_info_types_header_direct {
  args: "info --reload type_directives_01.ts",
  output: "info/types_header.out",
  http_server: true,
});
