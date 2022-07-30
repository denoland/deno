// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::process::Command;
use test_util::deno_cmd;

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
    .arg("run")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("--quiet")
    .arg(r#"--v8-flags=--allow-natives-syntax"#)
    .arg("tests/test.js")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
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
    false\n\
    true\n\
    false\n\
    false\n\
    false\n\
    579\n\
    true\n\
    579\n\
    579\n\
    5\n\
    5\n\
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
    After sleep_blocking\n\
    true\n\
    Before\n\
    true\n\
    After\n\
    true\n\
    logCallback\n\
    1 -1 2 -2 3 -3 4 -4 0.5 -0.5 1 2 3 4 5 6 7 8\n\
    u8: 8\n\
    buf: [1, 2, 3, 4, 5, 6, 7, 8]\n\
    logCallback\n\
    30\n\
    STORED_FUNCTION cleared\n\
    STORED_FUNCTION_2 cleared\n\
    Thread safe call counter: 0\n\
    logCallback\n\
    Thread safe call counter: 1\n\
    u8: 8\n\
    Static u32: 42\n\
    Static i64: -1242464576485\n\
    Static ptr: true\n\
    Static ptr value: 42\n\
    arrayBuffer.byteLength: 4\n\
    uint32Array.length: 1\n\
    uint32Array[0]: 42\n\
    uint32Array[0] after mutation: 55\n\
    Static ptr value after mutation: 55\n\
    Correct number of resources\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn symbol_types() {
  build();

  let output = deno_cmd()
    .arg("check")
    .arg("--unstable")
    .arg("--quiet")
    .arg("tests/ffi_types.ts")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  assert_eq!(stderr, "");
}

#[test]
fn thread_safe_callback() {
  build();

  let output = deno_cmd()
    .arg("run")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("--quiet")
    .arg("tests/thread_safe_test.js")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  println!("{:?}", output.status);
  assert!(output.status.success());
  let expected = "\
    Callback on main thread\n\
    Callback on worker thread\n\
    Calling callback, isolate should stay asleep until callback is called\n\
    Callback being called\n\
    Isolate should now exit\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}

#[test]
fn event_loop_integration() {
  build();

  let output = deno_cmd()
    .arg("run")
    .arg("--allow-ffi")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("--quiet")
    .arg("tests/event_loop_integration.ts")
    .env("NO_COLOR", "1")
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
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
    Timeout\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
