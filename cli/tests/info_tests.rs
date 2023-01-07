// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod integration;

use test_util as util;
use test_util::TempDir;

mod init {
  use super::*;

  #[test]
  fn info_with_compiled_source() {
    let _g = util::http_server();
    let module_path = "http://127.0.0.1:4545/run/048_media_types_jsx.ts";
    let t = TempDir::new();

    let mut deno = util::deno_cmd()
      .env("DENO_DIR", t.path())
      .current_dir(util::testdata_path())
      .arg("cache")
      .arg(module_path)
      .spawn()
      .unwrap();
    let status = deno.wait().unwrap();
    assert!(status.success());

    let output = util::deno_cmd()
      .env("DENO_DIR", t.path())
      .env("NO_COLOR", "1")
      .current_dir(util::testdata_path())
      .arg("info")
      .arg(module_path)
      .output()
      .unwrap();

    let str_output = std::str::from_utf8(&output.stdout).unwrap().trim();
    // check the output of the test.ts program.
    assert!(str_output.contains("emit: "));
    assert_eq!(output.stderr, b"");
  }

  itest!(multiple_imports {
    args: "info http://127.0.0.1:4545/run/019_media_types.ts",
    output: "info/multiple_imports.out",
    http_server: true,
  });

  itest!(info_ts_error {
    args: "info info/031_info_ts_error.ts",
    output: "info/031_info_ts_error.out",
  });

  itest!(info_flag {
    args: "info",
    output: "info/041_info_flag.out",
  });

  itest!(info_flag_location {
    args: "info --location https://deno.land",
    output: "info/041_info_flag_location.out",
  });

  itest!(info_json {
    args: "info --json --unstable",
    output: "info/info_json.out",
  });

  itest!(info_json_location {
    args: "info --json --unstable --location https://deno.land",
    output: "info/info_json_location.out",
  });

  itest!(info_flag_script_jsx {
    args: "info http://127.0.0.1:4545/run/048_media_types_jsx.ts",
    output: "info/049_info_flag_script_jsx.out",
    http_server: true,
  });

  itest!(json_file {
    args: "info --quiet --json --unstable info/json_output/main.ts",
    output: "info/json_output/main.out",
    exit_code: 0,
  });

  itest!(import_map_info {
    args:
      "info --quiet --import-map=import_maps/import_map.json import_maps/test.ts",
    output: "info/065_import_map_info.out",
  });

  itest!(info_json_deps_order {
    args: "info --unstable --json info/076_info_json_deps_order.ts",
    output: "info/076_info_json_deps_order.out",
  });

  itest!(info_missing_module {
    args: "info info/error_009_missing_js_module.js",
    output: "info/info_missing_module.out",
  });

  itest!(info_recursive_modules {
    args: "info --quiet info/info_recursive_imports_test.ts",
    output: "info/info_recursive_imports_test.out",
    exit_code: 0,
  });

  itest!(info_type_import {
    args: "info info/info_type_import.ts",
    output: "info/info_type_import.out",
  });

  itest!(_054_info_local_imports {
    args: "info --quiet run/005_more_imports.ts",
    output: "info/054_info_local_imports.out",
    exit_code: 0,
  });

  // Tests for AssertionError where "data" is unexpectedly null when
  // a file contains only triple slash references (#11196)
  itest!(data_null_error {
    args: "info info/data_null_error/mod.ts",
    output: "info/data_null_error/data_null_error.out",
  });

  itest!(types_header_direct {
    args: "info --reload run/type_directives_01.ts",
    output: "info/types_header.out",
    http_server: true,
  });

  itest!(with_config_override {
    args: "info info/with_config/test.ts --config info/with_config/deno-override.json --import-map info/with_config/import_map.json",
    output: "info/with_config/with_config.out",
  });
}
