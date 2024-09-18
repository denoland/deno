// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::process::Command;
use test_util::deno_cmd;
use test_util::deno_config_path;
use test_util::env_vars_for_npm_tests;
use test_util::http_server;
use test_util::napi_tests_path;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn build() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_napi");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(build_plugin_output.status.success());

  // cc module.c -undefined dynamic_lookup -shared -Wl,-no_fixup_chains -dynamic -o module.dylib
  #[cfg(not(target_os = "windows"))]
  {
    let out = if cfg!(target_os = "macos") {
      "module.dylib"
    } else {
      "module.so"
    };

    let mut cc = Command::new("cc");

    #[cfg(not(target_os = "macos"))]
    let c_module = cc.arg("module.c").arg("-shared").arg("-o").arg(out);

    #[cfg(target_os = "macos")]
    let c_module = {
      cc.arg("module.c")
        .arg("-undefined")
        .arg("dynamic_lookup")
        .arg("-shared")
        .arg("-Wl,-no_fixup_chains")
        .arg("-dynamic")
        .arg("-o")
        .arg(out)
    };
    let c_module_output = c_module.output().unwrap();
    assert!(c_module_output.status.success());
  }
}

#[test]
fn napi_tests() {
  build();

  let _http_guard = http_server();
  let output = deno_cmd()
    .current_dir(napi_tests_path())
    .env("RUST_BACKTRACE", "1")
    .arg("test")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-ffi")
    .arg("--allow-run")
    .arg("--v8-flags=--expose-gc")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg(".")
    .envs(env_vars_for_npm_tests())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  if !output.status.success() {
    eprintln!("exit code {:?}", output.status.code());
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
}
