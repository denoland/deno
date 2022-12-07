// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod integration;

use std::process::Command;
use std::process::Stdio;
use test_util as util;
use util::TempDir;

mod check {
  use super::*;
  itest!(_095_check_with_bare_import {
    args: "check cache/095_cache_with_bare_import.ts",
    output: "cache/095_cache_with_bare_import.ts.out",
    exit_code: 1,
  });

  itest!(check_extensionless {
    args: "check --reload http://localhost:4545/subdir/no_js_ext",
    output: "cache/cache_extensionless.out",
    http_server: true,
  });

  itest!(check_random_extension {
    args: "check --reload http://localhost:4545/subdir/no_js_ext@1.0.0",
    output: "cache/cache_random_extension.out",
    http_server: true,
  });

  itest!(check_all {
    args: "check --quiet --remote check/check_all.ts",
    output: "check/check_all.out",
    http_server: true,
    exit_code: 1,
  });

  itest!(check_all_local {
    args: "check --quiet check/check_all.ts",
    output_str: Some(""),
    http_server: true,
  });

  itest!(module_detection_force {
    args: "check --quiet check/module_detection_force/main.ts",
    output_str: Some(""),
  });

  // Regression test for https://github.com/denoland/deno/issues/14937.
  itest!(declaration_header_file_with_no_exports {
    args: "check --quiet check/declaration_header_file_with_no_exports.ts",
    output_str: Some(""),
  });

  itest!(check_npm_install_diagnostics {
    args: "check --quiet check/npm_install_diagnostics/main.ts",
    output: "check/npm_install_diagnostics/main.out",
    envs: vec![("NO_COLOR".to_string(), "1".to_string())],
    exit_code: 1,
  });

  itest!(check_export_equals_declaration_file {
    args: "check --quiet check/export_equals_declaration_file/main.ts",
    exit_code: 0,
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

  #[test]
  fn typecheck_declarations_ns() {
    let output = util::deno_cmd()
      .arg("test")
      .arg("--doc")
      .arg(util::root_path().join("cli/tsc/dts/lib.deno.ns.d.ts"))
      .output()
      .unwrap();
    println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
    println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    assert!(output.status.success());
  }

  #[test]
  fn typecheck_declarations_unstable() {
    let output = util::deno_cmd()
      .arg("test")
      .arg("--doc")
      .arg("--unstable")
      .arg(util::root_path().join("cli/tsc/dts/lib.deno.unstable.d.ts"))
      .output()
      .unwrap();
    println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
    println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    assert!(output.status.success());
  }

  #[test]
  fn typecheck_core() {
    let deno_dir = TempDir::new();
    let test_file = deno_dir.path().join("test_deno_core_types.ts");
    std::fs::write(
      &test_file,
      format!(
        "import \"{}\";",
        deno_core::resolve_path(
          util::root_path()
            .join("core/lib.deno_core.d.ts")
            .to_str()
            .unwrap()
        )
        .unwrap()
      ),
    )
    .unwrap();
    let output = util::deno_cmd_with_deno_dir(&deno_dir)
      .arg("run")
      .arg(test_file.to_str().unwrap())
      .output()
      .unwrap();
    println!("stdout: {}", String::from_utf8(output.stdout).unwrap());
    println!("stderr: {}", String::from_utf8(output.stderr).unwrap());
    assert!(output.status.success());
  }

  #[test]
  fn ts_no_recheck_on_redirect() {
    let deno_dir = util::new_deno_dir();
    let e = util::deno_exe_path();

    let redirect_ts = util::testdata_path().join("run/017_import_redirect.ts");
    assert!(redirect_ts.is_file());
    let mut cmd = Command::new(e.clone());
    cmd.env("DENO_DIR", deno_dir.path());
    let mut initial = cmd
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--check")
      .arg(redirect_ts.clone())
      .spawn()
      .expect("failed to span script");
    let status_initial =
      initial.wait().expect("failed to wait for child process");
    assert!(status_initial.success());

    let mut cmd = Command::new(e);
    cmd.env("DENO_DIR", deno_dir.path());
    let output = cmd
      .current_dir(util::testdata_path())
      .arg("run")
      .arg("--check")
      .arg(redirect_ts)
      .output()
      .expect("failed to spawn script");

    assert!(std::str::from_utf8(&output.stderr).unwrap().is_empty());
  }
}
