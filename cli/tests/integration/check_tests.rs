// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::process::Stdio;

use crate::itest;

use test_util as util;

itest!(_095_check_with_bare_import {
  args: "check 095_cache_with_bare_import.ts",
  output: "095_cache_with_bare_import.ts.out",
  exit_code: 1,
});

itest!(check_extensionless {
  args: "check --reload http://localhost:4545/subdir/no_js_ext",
  output: "cache_extensionless.out",
  http_server: true,
});

itest!(check_random_extension {
  args: "check --reload http://localhost:4545/subdir/no_js_ext@1.0.0",
  output: "cache_random_extension.out",
  http_server: true,
});

itest!(check_all {
  args: "check --quiet --remote check_all.ts",
  output: "check_all.out",
  http_server: true,
  exit_code: 1,
});

itest!(check_all_local {
  args: "check --quiet check_all.ts",
  output_str: Some(""),
  http_server: true,
});

itest!(module_detection_force {
  args: "check --quiet module_detection_force.ts",
  output_str: Some(""),
});

// Regression test for https://github.com/denoland/deno/issues/14937.
itest!(declaration_header_file_with_no_exports {
  args: "check --quiet declaration_header_file_with_no_exports.ts",
  output_str: Some(""),
});

#[test]
fn cache_switching_config_then_no_config() {
  let deno_dir = util::new_deno_dir();
  assert!(does_type_checking(&deno_dir, true));
  assert!(does_type_checking(&deno_dir, false));

  // should now not do type checking even when it changes
  // configs because it previously did
  assert!(!does_type_checking(&deno_dir, true));
  assert!(!does_type_checking(&deno_dir, false));

  fn does_type_checking(deno_dir: &util::TempDir, with_config: bool) -> bool {
    let mut cmd = util::deno_cmd_with_deno_dir(deno_dir);
    cmd
      .current_dir(util::testdata_path())
      .stderr(Stdio::piped())
      .arg("check")
      .arg("check/cache_config_on_off/main.ts");
    if with_config {
      cmd
        .arg("--config")
        .arg("check/cache_config_on_off/deno.json");
    }
    let output = cmd.spawn().unwrap().wait_with_output().unwrap();
    assert!(output.status.success());

    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    stderr.contains("Check")
  }
}

#[test]
fn reload_flag() {
  // should do type checking whenever someone specifies --reload
  let deno_dir = util::new_deno_dir();
  assert!(does_type_checking(&deno_dir, false));
  assert!(!does_type_checking(&deno_dir, false));
  assert!(does_type_checking(&deno_dir, true));
  assert!(does_type_checking(&deno_dir, true));
  assert!(!does_type_checking(&deno_dir, false));

  fn does_type_checking(deno_dir: &util::TempDir, reload: bool) -> bool {
    let mut cmd = util::deno_cmd_with_deno_dir(deno_dir);
    cmd
      .current_dir(util::testdata_path())
      .stderr(Stdio::piped())
      .arg("check")
      .arg("check/cache_config_on_off/main.ts");
    if reload {
      cmd.arg("--reload");
    }
    let output = cmd.spawn().unwrap().wait_with_output().unwrap();
    assert!(output.status.success());

    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    stderr.contains("Check")
  }
}
