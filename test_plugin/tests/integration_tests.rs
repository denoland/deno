// To run this test manually:
//   cd test_plugin
//   ../target/debug/deno run --unstable --allow-plugin tests/test.js debug

use std::path::PathBuf;
use std::process::Command;

fn target_dir() -> PathBuf {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  println!("target_dir {}", target_dir.display());
  target_dir.into()
}

fn deno_exe_path() -> PathBuf {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

fn deno_cmd() -> Command {
  assert!(deno_exe_path().exists());
  Command::new(deno_exe_path())
}

#[cfg(debug_assertions)]
const BUILD_VARIANT: &str = "debug";

#[cfg(not(debug_assertions))]
const BUILD_VARIANT: &str = "release";

#[test]
fn basic() {
  let mut build_plugin_base = Command::new("cargo");
  let mut build_plugin =
    build_plugin_base.arg("build").arg("-p").arg("test_plugin");
  if BUILD_VARIANT == "release" {
    build_plugin = build_plugin.arg("--release");
  }
  let build_plugin_output = build_plugin.output().unwrap();
  assert!(build_plugin_output.status.success());
  let output = deno_cmd()
    .arg("run")
    .arg("--allow-plugin")
    .arg("--unstable")
    .arg("tests/test.js")
    .arg(BUILD_VARIANT)
    .output()
    .unwrap();
  let stdout = std::str::from_utf8(&output.stdout).unwrap();
  let stderr = std::str::from_utf8(&output.stderr).unwrap();
  if !output.status.success() {
    println!("stdout {}", stdout);
    println!("stderr {}", stderr);
  }
  assert!(output.status.success());
  let expected = "Hello from plugin. data: test\nzero_copy[0]: test\nzero_copy[1]: 123\nzero_copy[2]: cba\nPlugin Sync Response: test\nHello from plugin. data: test\nzero_copy[0]: test\nzero_copy[1]: 123\nPlugin Async Response: test\n";
  assert_eq!(stdout, expected);
  assert_eq!(stderr, "");
}
