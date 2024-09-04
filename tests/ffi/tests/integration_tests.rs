// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use pretty_assertions::assert_eq;
use std::process::Command;
use test_util::deno_cmd;
use test_util::deno_config_path;
use test_util::ffi_tests_path;

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

fn build() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_ffi");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(build_plugin_output.status.success());
}

#[test]
fn basic() {
  build();

  let output = deno_cmd()
    .current_dir(ffi_tests_path())
    .arg("run")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable-ffi")
    .arg("--quiet")
    .arg(r#"--v8-flags=--allow-natives-syntax"#)
    .arg("tests/test.js")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {stdout}");
    println!("stderr {stderr}");
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  let expected = "\
    something\n\
    [1, 2, 3, 4, 5, 6, 7, 8]\n\
    [4, 5, 6]\n\
    [1, 2, 3, 4, 5, 6, 7, 8] [9, 10]\n\
    [1, 2, 3, 4, 5, 6, 7, 8]\n\
    [ 1, 2, 3, 4, 5, 6 ]\n\
    [ 4, 5, 6 ]\n\
    [ 4, 5, 6 ]\n\
    Hello from pointer!\n\
    pointer!\n\
    false false\n\
    true true\n\
    false false\n\
    true true\n\
    false false\n\
    579\n\
    true\n\
    579\n\
    579\n\
    5\n\
    5\n\
    579\n\
    8589934590n\n\
    -8589934590n\n\
    8589934590n\n\
    -8589934590n\n\
    9007199254740992n\n\
    9007199254740992n\n\
    -9007199254740992n\n\
    9007199254740992n\n\
    9007199254740992n\n\
    -9007199254740992n\n\
    579.9119873046875\n\
    579.912\n\
    true\n\
    false\n\
    579.9119873046875\n\
    579.9119873046875\n\
    579.912\n\
    579.912\n\
    579\n\
    8589934590\n\
    -8589934590\n\
    8589934590\n\
    -8589934590\n\
    9007199254740992n\n\
    9007199254740992n\n\
    -9007199254740992n\n\
    9007199254740992n\n\
    9007199254740992n\n\
    -9007199254740992n\n\
    579.9119873046875\n\
    579.912\n\
    Before\n\
    After\n\
    logCallback\n\
    1 -1 2 -2 3 -3 4n -4n 0.5 -0.5 1 2 3 4 5 6 7 8\n\
    u8: 8\n\
    buf: [1, 2, 3, 4, 5, 6, 7, 8]\n\
    logCallback\n\
    30\n\
    255 65535 4294967295 4294967296 123.456 789.876 -1 -2 -3 -4 -1000 1000 12345.67891 12345.679 12345.67891 12345.679 12345.67891 12345.679 12345.67891\n\
    255 65535 4294967295 4294967296 123.456 789.876 -1 -2 -3 -4 -1000 1000 12345.67891 12345.679 12345.67891 12345.679 12345.67891 12345.679 12345.67891\n\
    0\n\
    0\n\
    0\n\
    0\n\
    78\n\
    78\n\
    STORED_FUNCTION cleared\n\
    STORED_FUNCTION_2 cleared\n\
    logCallback\n\
    u8: 8\n\
    Rect { x: 10.0, y: 20.0, w: 100.0, h: 200.0 }\n\
    Rect { x: 10.0, y: 20.0, w: 100.0, h: 200.0 }\n\
    Rect { x: 20.0, y: 20.0, w: 100.0, h: 200.0 }\n\
    Mixed { u8: 3, f32: 12.515, rect: Rect { x: 10.0, y: 20.0, w: 100.0, h: 200.0 }, usize: 12456789, array: [8, 32] }\n\
    2264956937\n\
    2264956937\n\
    Correct number of resources\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn symbol_types() {
  build();

  let output = deno_cmd()
    .current_dir(ffi_tests_path())
    .arg("check")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--unstable-ffi")
    .arg("--quiet")
    .arg("tests/ffi_types.ts")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {stdout}");
    println!("stderr {stderr}");
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  assert_eq!(stderr, "");
}

#[test]
fn thread_safe_callback() {
  build();

  let output = deno_cmd()
    .current_dir(ffi_tests_path())
    .arg("run")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable-ffi")
    .arg("--quiet")
    .arg("tests/thread_safe_test.js")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {stdout}");
    println!("stderr {stderr}");
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  let expected = "\
    Callback on main thread\n\
    Callback on worker thread\n\
    STORED_FUNCTION cleared\n\
    Calling callback, isolate should stay asleep until callback is called\n\
    Callback being called\n\
    STORED_FUNCTION cleared\n\
    Isolate should now exit\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn event_loop_integration() {
  build();

  let output = deno_cmd()
    .current_dir(ffi_tests_path())
    .arg("run")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable-ffi")
    .arg("--quiet")
    .arg("tests/event_loop_integration.ts")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {stdout}");
    println!("stderr {stderr}");
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  // TODO(aapoalas): The order of logging in thread safe callbacks is
  // unexpected: The callback logs synchronously and creates an asynchronous
  // logging task, which then gets called synchronously before the callback
  // actually yields to the calling thread. This is in contrast to what the
  // logging would look like if the call was coming from within Deno itself,
  // and may lead users to unknowingly run heavy asynchronous tasks from thread
  // safe callbacks synchronously.
  // The fix would be to make sure microtasks are only run after the event loop
  // middleware that polls them has completed its work. This just does not seem
  // to work properly with Linux release builds.
  let expected = "\
    SYNCHRONOUS\n\
    Sync\n\
    STORED_FUNCTION called\n\
    Async\n\
    Timeout\n\
    THREAD SAFE\n\
    Sync\n\
    Async\n\
    STORED_FUNCTION called\n\
    Timeout\n\
    RETRY THREAD SAFE\n\
    Sync\n\
    Async\n\
    STORED_FUNCTION called\n\
    Timeout\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn ffi_callback_errors_test() {
  build();

  let output = deno_cmd()
    .current_dir(ffi_tests_path())
    .arg("run")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable-ffi")
    .arg("--quiet")
    .arg("tests/ffi_callback_errors.ts")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {stdout}");
    println!("stderr {stderr}");
  }
  println!("{:?}", output.status);
  assert!(output.status.success());

  let expected = "\
    CallCase: SyncSelf\n\
    Throwing errors from an UnsafeCallback called from a synchronous UnsafeFnPointer works. Terribly excellent.\n\
    CallCase: SyncFfi\n\
    0\n\
    Throwing errors from an UnsafeCallback called from a synchronous FFI symbol works. Terribly excellent.\n\
    CallCase: AsyncSelf\n\
    CallCase: AsyncSyncFfi\n\
    0\n\
    Calling\n\
    CallCase: AsyncFfi\n";
  assert_eq!(stdout, expected);
  assert_eq!(
    stderr,
    "Illegal unhandled exception in nonblocking callback\n".repeat(3)
  );
}
