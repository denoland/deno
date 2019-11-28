use deno_cli::test_util::*;
use std::process::Command;

fn deno_cmd() -> Command {
  Command::new(deno_exe_path())
}

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

#[test]
fn basic() {
  let output = deno_cmd()
    .arg("--allow-plugin")
    .arg("tests/test.js")
    .arg(BUILD_VARIANT)
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  //println!("stdout {:?}", stdout);
  //println!("stderr {:?}", stderr);
  if !output.status.success() {
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
  let expected = "Hello from native bindings. data: test | zero_copy: test\nNative Binding Sync Response: test\nHello from native bindings. data: test | zero_copy: test\nNative Binding Async Response: test\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
