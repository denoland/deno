// Copyright 2018-2026 the Deno authors. MIT license.

use std::process::Command;

use test_util::deno_cmd;
use test_util::deno_config_path;
use test_util::env_vars_for_npm_tests;
use test_util::eprintln;
use test_util::http_server;
use test_util::napi_tests_path;
use test_util::println;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn napi_build() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_napi");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(
    build_plugin_output.status.success(),
    "cargo build failed:\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&build_plugin_output.stdout),
    String::from_utf8_lossy(&build_plugin_output.stderr)
  );

  // cc module.c -undefined dynamic_lookup -shared -Wl,-no_fixup_chains -dynamic -o module.dylib
  #[cfg(not(target_os = "windows"))]
  {
    let out = if cfg!(target_os = "macos") {
      "module.dylib"
    } else {
      "module.so"
    };

    let mut cc = Command::new("cc");
    cc.current_dir(napi_tests_path());

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
    assert!(
      c_module_output.status.success(),
      "cc failed:\nstdout: {}\nstderr: {}",
      String::from_utf8_lossy(&c_module_output.stdout),
      String::from_utf8_lossy(&c_module_output.stderr)
    );
  }
}

#[test_util::test]
fn napi_tests() {
  napi_build();

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
    .output()
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

/// Test that NAPI wrap finalizers are called at shutdown even when the
/// wrapped JS object is still reachable. This matches Node.js behavior
/// and is required for native addons like DuckDB that rely on destructor
/// cleanup (e.g., WAL checkpointing) during process exit.
#[test_util::test]
fn napi_wrap_leak_pointers_finalizer_on_shutdown() {
  napi_build();

  let output = deno_cmd()
    .current_dir(napi_tests_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-ffi")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("wrap_leak.js")
    .envs(env_vars_for_npm_tests())
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  if !output.status.success() {
    eprintln!("exit code {:?}", output.status.code());
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
  assert!(
    stdout.contains("pointers released on shutdown"),
    "Expected wrap finalizer to run at shutdown, got stdout: {}",
    stdout
  );
}

/// Test napi_fatal_error: calling it should abort the process and log the
/// error message to stderr.
#[test_util::test]
fn napi_fatal_error() {
  napi_build();

  let output = deno_cmd()
    .current_dir(napi_tests_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-ffi")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("fatal_error.js")
    .envs(env_vars_for_npm_tests())
    .output()
    .unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  // Process should have been killed (abort signal)
  assert!(
    !output.status.success(),
    "Expected process to abort, but it exited successfully"
  );
  assert!(
    stderr.contains("NODE API FATAL ERROR"),
    "Expected fatal error message in stderr, got: {}",
    stderr
  );
}

/// Test napi_fatal_exception: calling it should trigger the uncaught
/// exception handler and exit with a non-zero code.
#[test_util::test]
fn napi_fatal_exception() {
  napi_build();

  let output = deno_cmd()
    .current_dir(napi_tests_path())
    .arg("run")
    .arg("--allow-read")
    .arg("--allow-env")
    .arg("--allow-ffi")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("fatal_exception.js")
    .envs(env_vars_for_npm_tests())
    .output()
    .unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();

  assert!(
    !output.status.success(),
    "Expected process to exit with error, but it succeeded"
  );
  assert!(
    stderr.contains("fatal exception test"),
    "Expected error message in stderr, got: {}",
    stderr
  );
}
