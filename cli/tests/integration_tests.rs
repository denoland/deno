// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#[cfg(unix)]
extern crate nix;
extern crate tempfile;

use test_util as util;

use futures::prelude::*;
use std::io::{BufRead, Write};
use std::process::Command;
use tempfile::TempDir;

#[test]
fn std_tests() {
  let dir = TempDir::new().expect("tempdir fail");
  let std_path = util::root_path().join("std");
  let std_config = std_path.join("tsconfig_test.json");
  let status = util::deno_cmd()
    .env("DENO_DIR", dir.path())
    .current_dir(std_path) // TODO(ry) change this to root_path
    .arg("test")
    .arg("--unstable")
    .arg("--seed=86") // Some tests rely on specific random numbers.
    .arg("-A")
    .arg("--config")
    .arg(std_config.to_str().unwrap())
    // .arg("-Ldebug")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn std_lint() {
  let status = util::deno_cmd()
    .arg("lint")
    .arg("--unstable")
    .arg(util::root_path().join("std"))
    .arg(util::root_path().join("cli/tests/unit"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn x_deno_warning() {
  let _g = util::http_server();
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--reload")
    .arg("http://127.0.0.1:4545/cli/tests/x_deno_warning.js")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_str = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert_eq!("testing x-deno-warning header", stdout_str);
  assert!(util::strip_ansi_codes(stderr_str).contains("Warning foobar"));
}

#[test]
fn eval_p() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("-p")
    .arg("1+2")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap().trim());
  assert_eq!("3", stdout_str);
}

#[test]

fn run_from_stdin() {
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("-")
    .stdout(std::process::Stdio::piped())
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  deno
    .stdin
    .as_mut()
    .unwrap()
    .write_all(b"console.log(\"Hello World\");")
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  let deno_out = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!("Hello World", deno_out);

  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("-")
    .stdout(std::process::Stdio::piped())
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  deno
    .stdin
    .as_mut()
    .unwrap()
    .write_all(b"console.log(\"Bye cached code\");")
    .unwrap();
  let output = deno.wait_with_output().unwrap();
  assert!(output.status.success());

  let deno_out = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!("Bye cached code", deno_out);
}

#[test]
fn no_color() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("cli/tests/no_color.js")
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!("noColor true", stdout_str);

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("cli/tests/no_color.js")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert_eq!("noColor false", util::strip_ansi_codes(stdout_str));
}

// TODO re-enable. This hangs on macOS
// https://github.com/denoland/deno/issues/4262
#[cfg(unix)]
#[test]
#[ignore]
pub fn test_raw_tty() {
  use std::io::{Read, Write};
  use util::pty::fork::*;

  let fork = Fork::from_ptmx().unwrap();

  if let Ok(mut master) = fork.is_parent() {
    let mut obytes: [u8; 100] = [0; 100];
    let mut nread = master.read(&mut obytes).unwrap();
    assert_eq!(String::from_utf8_lossy(&obytes[0..nread]), "S");
    master.write_all(b"a").unwrap();
    nread = master.read(&mut obytes).unwrap();
    assert_eq!(String::from_utf8_lossy(&obytes[0..nread]), "A");
    master.write_all(b"b").unwrap();
    nread = master.read(&mut obytes).unwrap();
    assert_eq!(String::from_utf8_lossy(&obytes[0..nread]), "B");
    master.write_all(b"c").unwrap();
    nread = master.read(&mut obytes).unwrap();
    assert_eq!(String::from_utf8_lossy(&obytes[0..nread]), "C");
  } else {
    use nix::sys::termios;
    use std::os::unix::io::AsRawFd;
    use std::process::*;

    // Turn off echo such that parent is reading works properly.
    let stdin_fd = std::io::stdin().as_raw_fd();
    let mut t = termios::tcgetattr(stdin_fd).unwrap();
    t.local_flags.remove(termios::LocalFlags::ECHO);
    termios::tcsetattr(stdin_fd, termios::SetArg::TCSANOW, &t).unwrap();

    let deno_dir = TempDir::new().expect("tempdir fail");
    let mut child = Command::new(util::deno_exe_path())
      .env("DENO_DIR", deno_dir.path())
      .current_dir(util::root_path())
      .arg("run")
      .arg("cli/tests/raw_mode.ts")
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::null())
      .spawn()
      .expect("Failed to spawn script");
    child.wait().unwrap();
  }
}

#[test]
fn test_pattern_match() {
  // foo, bar, baz, qux, quux, quuz, corge, grault, garply, waldo, fred, plugh, xyzzy

  let wildcard = "[BAR]";
  assert!(util::pattern_match("foo[BAR]baz", "foobarbaz", wildcard));
  assert!(!util::pattern_match("foo[BAR]baz", "foobazbar", wildcard));

  let multiline_pattern = "[BAR]
foo:
[BAR]baz[BAR]";

  fn multi_line_builder(input: &str, leading_text: Option<&str>) -> String {
    // If there is leading text add a newline so it's on it's own line
    let head = match leading_text {
      Some(v) => format!("{}\n", v),
      None => "".to_string(),
    };
    format!(
      "{}foo:
quuz {} corge
grault",
      head, input
    )
  }

  // Validate multi-line string builder
  assert_eq!(
    "QUUX=qux
foo:
quuz BAZ corge
grault",
    multi_line_builder("BAZ", Some("QUUX=qux"))
  );

  // Correct input & leading line
  assert!(util::pattern_match(
    multiline_pattern,
    &multi_line_builder("baz", Some("QUX=quux")),
    wildcard
  ));

  // Correct input & no leading line
  assert!(util::pattern_match(
    multiline_pattern,
    &multi_line_builder("baz", None),
    wildcard
  ));

  // Incorrect input & leading line
  assert!(!util::pattern_match(
    multiline_pattern,
    &multi_line_builder("garply", Some("QUX=quux")),
    wildcard
  ));

  // Incorrect input & no leading line
  assert!(!util::pattern_match(
    multiline_pattern,
    &multi_line_builder("garply", None),
    wildcard
  ));
}

#[test]
fn benchmark_test() {
  util::run_python_script("tools/benchmark_test.py")
}

#[test]
fn deno_dir_test() {
  use std::fs::remove_dir_all;
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  remove_dir_all(deno_dir.path()).unwrap();

  // Run deno with no env flag
  let status = util::deno_cmd()
    .env_remove("DENO_DIR")
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://localhost:4545/cli/tests/subdir/print_hello.ts")
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(status.success());
  assert!(!deno_dir.path().exists());

  // Run deno with DENO_DIR env flag
  let status = util::deno_cmd()
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::root_path())
    .arg("run")
    .arg("http://localhost:4545/cli/tests/subdir/print_hello.ts")
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(status.success());
  assert!(deno_dir.path().is_dir());
  assert!(deno_dir.path().join("deps").is_dir());
  assert!(deno_dir.path().join("gen").is_dir());

  remove_dir_all(deno_dir.path()).unwrap();
}

#[test]
fn cache_test() {
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    url::Url::parse("http://localhost:4545/cli/tests/006_url_imports.ts")
      .unwrap();
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::root_path())
    .arg("cache")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let out = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(out, "");
  // TODO(ry) Is there some way to check that the file was actually cached in
  // DENO_DIR?
}

#[test]
fn fmt_test() {
  let t = TempDir::new().expect("tempdir fail");
  let fixed = util::root_path().join("cli/tests/badly_formatted_fixed.js");
  let badly_formatted_original =
    util::root_path().join("cli/tests/badly_formatted.mjs");
  let badly_formatted = t.path().join("badly_formatted.js");
  let badly_formatted_str = badly_formatted.to_str().unwrap();
  std::fs::copy(&badly_formatted_original, &badly_formatted)
    .expect("Failed to copy file");
  // First, check formatting by ignoring the badly formatted file.
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("fmt")
    .arg(format!("--ignore={}", badly_formatted_str))
    .arg("--unstable")
    .arg("--check")
    .arg(badly_formatted_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(status.success());
  // Check without ignore.
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("fmt")
    .arg("--check")
    .arg(badly_formatted_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(!status.success());
  // Format the source file.
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("fmt")
    .arg(badly_formatted_str)
    .spawn()
    .expect("Failed to spawn script")
    .wait()
    .expect("Failed to wait for child process");
  assert!(status.success());
  let expected = std::fs::read_to_string(fixed).unwrap();
  let actual = std::fs::read_to_string(badly_formatted).unwrap();
  assert_eq!(expected, actual);
}

#[test]
fn fmt_stdin_error() {
  use std::io::Write;
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("fmt")
    .arg("-")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  let stdin = deno.stdin.as_mut().unwrap();
  let invalid_js = b"import { example }";
  stdin.write_all(invalid_js).unwrap();
  let output = deno.wait_with_output().unwrap();
  // Error message might change. Just check stdout empty, stderr not.
  assert!(output.stdout.is_empty());
  assert!(!output.stderr.is_empty());
  assert!(!output.status.success());
}

// Warning: this test requires internet access.
#[test]
fn upgrade_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let _mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let _mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  // TODO(ry) assert!(mtime1 < mtime2);
}

// Warning: this test requires internet access.
#[test]
fn upgrade_with_space_in_path() {
  let temp_dir = tempfile::Builder::new()
    .prefix("directory with spaces")
    .tempdir()
    .unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .env("TMP", temp_dir.path())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

// Warning: this test requires internet access.
#[test]
fn upgrade_with_version_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let _mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--force")
    .arg("--version")
    .arg("0.42.0")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let upgraded_deno_version = String::from_utf8(
    Command::new(&exe_path).arg("-V").output().unwrap().stdout,
  )
  .unwrap();
  assert!(upgraded_deno_version.contains("0.42.0"));
  let _mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  // TODO(ry) assert!(mtime1 < mtime2);
}

// Warning: this test requires internet access.
#[test]
fn upgrade_with_out_in_tmpdir() {
  let temp_dir = TempDir::new().unwrap();
  let exe_path = temp_dir.path().join("deno");
  let new_exe_path = temp_dir.path().join("foo");
  let _ = std::fs::copy(util::deno_exe_path(), &exe_path).unwrap();
  assert!(exe_path.exists());
  let mtime1 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  let status = Command::new(&exe_path)
    .arg("upgrade")
    .arg("--version")
    .arg("1.0.2")
    .arg("--output")
    .arg(&new_exe_path.to_str().unwrap())
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  assert!(new_exe_path.exists());
  let mtime2 = std::fs::metadata(&exe_path).unwrap().modified().unwrap();
  assert_eq!(mtime1, mtime2); // Original exe_path was not changed.

  let v = String::from_utf8(
    Command::new(&new_exe_path)
      .arg("-V")
      .output()
      .unwrap()
      .stdout,
  )
  .unwrap();
  assert!(v.contains("1.0.2"));
}

#[test]
fn installer_test_local_module_run() {
  let temp_dir = TempDir::new().expect("tempdir fail");
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("--root")
    .arg(temp_dir.path())
    .arg(util::tests_path().join("echo.ts"))
    .arg("hello")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let mut file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
  // NOTE: using file_path here instead of exec_name, because tests
  // shouldn't mess with user's PATH env variable
  let output = Command::new(file_path)
    .current_dir(temp_dir.path())
    .arg("foo")
    .env("PATH", util::target_dir())
    .output()
    .expect("failed to spawn script");
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout_str.ends_with("hello, foo"));
}

#[test]
fn installer_test_remote_module_run() {
  let _g = util::http_server();
  let temp_dir = TempDir::new().expect("tempdir fail");
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let status = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("install")
    .arg("--name")
    .arg("echo_test")
    .arg("--root")
    .arg(temp_dir.path())
    .arg("http://localhost:4545/cli/tests/echo.ts")
    .arg("hello")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
  let mut file_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }
  assert!(file_path.exists());
  let output = Command::new(file_path)
    .current_dir(temp_dir.path())
    .arg("foo")
    .env("PATH", util::target_dir())
    .output()
    .expect("failed to spawn script");
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("hello, foo"));
}

#[test]
fn js_unit_tests() {
  let _g = util::http_server();
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--unstable")
    .arg("--reload")
    .arg("-A")
    .arg("cli/tests/unit/unit_test_runner.ts")
    .arg("--master")
    .arg("--verbose")
    .env("NO_COLOR", "1")
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert_eq!(Some(0), status.code());
  assert!(status.success());
}

#[test]
fn ts_dependency_recompilation() {
  let t = TempDir::new().expect("tempdir fail");
  let ats = t.path().join("a.ts");

  std::fs::write(
    &ats,
    "
    import { foo } from \"./b.ts\";

    function print(str: string): void {
        console.log(str);
    }

    print(foo);",
  )
  .unwrap();

  let bts = t.path().join("b.ts");
  std::fs::write(
    &bts,
    "
    export const foo = \"foo\";",
  )
  .unwrap();

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  assert!(stdout_output.ends_with("foo"));
  assert!(stderr_output.starts_with("Check"));

  // Overwrite contents of b.ts and run again
  std::fs::write(
    &bts,
    "
    export const foo = 5;",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .env("NO_COLOR", "1")
    .arg("run")
    .arg(&ats)
    .output()
    .expect("failed to spawn script");

  let stdout_output = std::str::from_utf8(&output.stdout).unwrap().trim();
  let stderr_output = std::str::from_utf8(&output.stderr).unwrap().trim();

  // error: TS2345 [ERROR]: Argument of type '5' is not assignable to parameter of type 'string'.
  assert!(stderr_output.contains("TS2345"));
  assert!(!output.status.success());
  assert!(stdout_output.is_empty());
}

#[test]
fn ts_reload() {
  let hello_ts = util::root_path().join("cli/tests/002_hello.ts");
  assert!(hello_ts.is_file());
  let mut initial = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("cache")
    .arg("--reload")
    .arg(hello_ts.clone())
    .spawn()
    .expect("failed to spawn script");
  let status_initial =
    initial.wait().expect("failed to wait for child process");
  assert!(status_initial.success());

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("cache")
    .arg("--reload")
    .arg("-L")
    .arg("debug")
    .arg(hello_ts)
    .output()
    .expect("failed to spawn script");
  // check the output of the the bundle program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .contains("\"compiler::host.writeFile\" \"deno://002_hello.js\""));
}

#[test]
fn bundle_exports() {
  // First we have to generate a bundle of some module that has exports.
  let mod1 = util::root_path().join("cli/tests/subdir/mod1.ts");
  assert!(mod1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(mod1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_circular() {
  // First we have to generate a bundle of some module that has exports.
  let circular1 = util::root_path().join("cli/tests/subdir/circular1.ts");
  assert!(circular1.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("circular1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(circular1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the the bundle program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("f1\nf2"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_single_module() {
  // First we have to generate a bundle of some module that has exports.
  let single_module =
    util::root_path().join("cli/tests/subdir/single_module.ts");
  assert!(single_module.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("single_module.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(single_module)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--reload")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the the bundle program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello world!"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_tla() {
  // First we have to generate a bundle of some module that has exports.
  let tla_import = util::root_path().join("cli/tests/subdir/tla.ts");
  assert!(tla_import.is_file());
  let t = tempfile::TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("tla.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(tla_import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { foo } from \"./tla.bundle.js\";
      console.log(foo); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_js() {
  // First we have to generate a bundle of some module that has exports.
  let mod6 = util::root_path().join("cli/tests/subdir/mod6.js");
  assert!(mod6.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod6.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(mod6)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check that nothing went to stderr
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_dynamic_import() {
  let dynamic_import =
    util::root_path().join("cli/tests/subdir/subdir2/dynamic_import.ts");
  assert!(dynamic_import.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("dynamic_import.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg(dynamic_import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&bundle)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn bundle_import_map() {
  let import = util::root_path().join("cli/tests/bundle_im.ts");
  let import_map_path = util::root_path().join("cli/tests/bundle_im.json");
  assert!(import.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("import_map.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg("--importmap")
    .arg(import_map_path)
    .arg("--unstable")
    .arg(import)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./import_map.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn repl_test_console_log() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["console.log('hello')", "'world'"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("hello\nundefined\nworld\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_cwd() {
  let (_out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["Deno.cwd()"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(err.is_empty());
}

#[test]
fn repl_test_eof() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1 + 2"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("3\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_strict() {
  let (_, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "let a = {};",
      "Object.preventExtensions(a);",
      "a.c = 1;",
    ]),
    None,
    false,
  );
  assert!(err.contains(
    "Uncaught TypeError: Cannot add property c, object is not extensible"
  ));
}

const REPL_MSG: &str = "exit using ctrl+d or close()\n";

#[test]
fn repl_test_close_command() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["close()", "'ignored'"]),
    None,
    false,
  );

  assert!(!out.contains("ignored"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_function() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["Deno.writeFileSync"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("[Function: writeFileSync]\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_multiline() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["(\n1 + 2\n)"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("3\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_eval_unterminated() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["eval('{')"]),
    None,
    false,
  );
  assert!(out.ends_with(REPL_MSG));
  assert!(err.contains("Unexpected end of input"));
}

#[test]
fn repl_test_reference_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["not_a_variable"]),
    None,
    false,
  );
  assert!(out.ends_with(REPL_MSG));
  assert!(err.contains("not_a_variable is not defined"));
}

#[test]
fn repl_test_syntax_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["syntax error"]),
    None,
    false,
  );
  assert!(out.ends_with(REPL_MSG));
  assert!(err.contains("Unexpected identifier"));
}

#[test]
fn repl_test_type_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["console()"]),
    None,
    false,
  );
  assert!(out.ends_with(REPL_MSG));
  assert!(err.contains("console is not a function"));
}

#[test]
fn repl_test_variable() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["var a = 123;", "a"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n123\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_lexical_scoped_variable() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["let a = 123;", "a"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n123\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_missing_deno_dir() {
  use std::fs::{read_dir, remove_dir_all};
  const DENO_DIR: &str = "nonexistent";
  let test_deno_dir =
    util::root_path().join("cli").join("tests").join(DENO_DIR);
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1"]),
    Some(vec![
      ("DENO_DIR".to_owned(), DENO_DIR.to_owned()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert!(read_dir(&test_deno_dir).is_ok());
  remove_dir_all(&test_deno_dir).unwrap();
  assert!(out.ends_with("1\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_save_last_eval() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1", "_"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("1\n1\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_test_save_last_thrown() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["throw 1", "_error"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("1\n"));
  assert_eq!(err, "Thrown: 1\n");
}

#[test]
fn repl_test_assign_underscore() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["_ = 1", "2", "_"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(
    out.ends_with("Last evaluation result is no longer saved to _.\n1\n2\n1\n")
  );
  assert!(err.is_empty());
}

#[test]
fn repl_test_assign_underscore_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["_error = 1", "throw 2", "_error"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(
    out.ends_with("Last thrown error is no longer saved to _error.\n1\n1\n")
  );
  assert_eq!(err, "Thrown: 2\n");
}

#[test]
fn deno_test_no_color() {
  let (out, _) = util::run_and_collect_output(
    false,
    "test deno_test_no_color.ts",
    None,
    Some(vec![("NO_COLOR".to_owned(), "true".to_owned())]),
    false,
  );
  // ANSI escape codes should be stripped.
  assert!(out.contains("test success ... ok"));
  assert!(out.contains("test fail ... FAILED"));
  assert!(out.contains("test ignored ... ignored"));
  assert!(out.contains("test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out"));
}

#[test]
fn util_test() {
  util::run_python_script("tools/util_test.py")
}

macro_rules! itest(
  ($name:ident {$( $key:ident: $value:expr,)*})  => {
    #[test]
    fn $name() {
      (util::CheckOutputIntegrationTest {
        $(
          $key: $value,
         )*
        .. Default::default()
      }).run()
    }
  }
);

// Unfortunately #[ignore] doesn't work with itest!
#[allow(unused)]
macro_rules! itest_ignore(
  ($name:ident {$( $key:ident: $value:expr,)*})  => {
    #[ignore]
    #[test]
    fn $name() {
      (util::CheckOutputIntegrationTest {
        $(
          $key: $value,
         )*
        .. Default::default()
      }).run()
    }
  }
);

itest!(_001_hello {
  args: "run --reload 001_hello.js",
  output: "001_hello.js.out",
});

itest!(_002_hello {
  args: "run --quiet --reload 002_hello.ts",
  output: "002_hello.ts.out",
});

itest!(_003_relative_import {
  args: "run --quiet --reload 003_relative_import.ts",
  output: "003_relative_import.ts.out",
});

itest!(_004_set_timeout {
  args: "run --quiet --reload 004_set_timeout.ts",
  output: "004_set_timeout.ts.out",
});

itest!(_005_more_imports {
  args: "run --quiet --reload 005_more_imports.ts",
  output: "005_more_imports.ts.out",
});

itest!(_006_url_imports {
  args: "run --quiet --reload 006_url_imports.ts",
  output: "006_url_imports.ts.out",
  http_server: true,
});

itest!(_012_async {
  args: "run --quiet --reload 012_async.ts",
  output: "012_async.ts.out",
});

itest!(_013_dynamic_import {
  args: "run --quiet --reload --allow-read 013_dynamic_import.ts",
  output: "013_dynamic_import.ts.out",
});

itest!(_014_duplicate_import {
  args: "run --quiet --reload --allow-read 014_duplicate_import.ts ",
  output: "014_duplicate_import.ts.out",
});

itest!(_015_duplicate_parallel_import {
  args: "run --quiet --reload --allow-read 015_duplicate_parallel_import.js",
  output: "015_duplicate_parallel_import.js.out",
});

itest!(_016_double_await {
  args: "run --quiet --allow-read --reload 016_double_await.ts",
  output: "016_double_await.ts.out",
});

itest!(_017_import_redirect {
  args: "run --quiet --reload 017_import_redirect.ts",
  output: "017_import_redirect.ts.out",
});

itest!(_018_async_catch {
  args: "run --quiet --reload 018_async_catch.ts",
  output: "018_async_catch.ts.out",
});

itest!(_019_media_types {
  args: "run --reload 019_media_types.ts",
  output: "019_media_types.ts.out",
  http_server: true,
});

itest!(_020_json_modules {
  args: "run --reload 020_json_modules.ts",
  output: "020_json_modules.ts.out",
  exit_code: 1,
});

itest!(_021_mjs_modules {
  args: "run --quiet --reload 021_mjs_modules.ts",
  output: "021_mjs_modules.ts.out",
});

itest!(_022_info_flag_script {
  args: "info http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "022_info_flag_script.out",
  http_server: true,
});

itest!(_023_no_ext_with_headers {
  args: "run --reload 023_no_ext_with_headers",
  output: "023_no_ext_with_headers.out",
});

// TODO(lucacasonato): remove --unstable when permissions goes stable
itest!(_025_hrtime {
  args: "run --quiet --allow-hrtime --unstable --reload 025_hrtime.ts",
  output: "025_hrtime.ts.out",
});

itest!(_025_reload_js_type_error {
  args: "run --quiet --reload 025_reload_js_type_error.js",
  output: "025_reload_js_type_error.js.out",
});

itest!(_026_redirect_javascript {
  args: "run --quiet --reload 026_redirect_javascript.js",
  output: "026_redirect_javascript.js.out",
  http_server: true,
});

itest!(deno_test {
  args: "test test_runner_test.ts",
  exit_code: 1,
  output: "deno_test.out",
});

itest!(deno_test_fail_fast {
  args: "test --failfast test_runner_test.ts",
  exit_code: 1,
  output: "deno_test_fail_fast.out",
});

itest!(deno_test_only {
  args: "test deno_test_only.ts",
  exit_code: 1,
  output: "deno_test_only.ts.out",
});

#[test]
fn workers() {
  let _g = util::http_server();
  let status = util::deno_cmd()
    .current_dir(util::tests_path())
    .arg("test")
    .arg("--reload")
    .arg("--allow-net")
    .arg("--allow-read")
    .arg("--unstable")
    .arg("workers_test.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

#[test]
fn compiler_api() {
  let status = util::deno_cmd()
    .current_dir(util::tests_path())
    .arg("test")
    .arg("--unstable")
    .arg("--reload")
    .arg("--allow-read")
    .arg("compiler_api_test.ts")
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

itest!(_027_redirect_typescript {
  args: "run --quiet --reload 027_redirect_typescript.ts",
  output: "027_redirect_typescript.ts.out",
  http_server: true,
});

itest!(_028_args {
  args: "run --quiet --reload 028_args.ts --arg1 val1 --arg2=val2 -- arg3 arg4",
  output: "028_args.ts.out",
});

itest!(_029_eval {
  args: "eval console.log(\"hello\")",
  output: "029_eval.out",
});

// Ugly parentheses due to whitespace delimiting problem.
itest!(_030_eval_ts {
  args: "eval --quiet -T console.log((123)as(number))", // 'as' is a TS keyword only
  output: "030_eval_ts.out",
});

itest!(_031_info_no_check {
  args: "info 031_info_no_check.ts",
  output: "031_info_no_check.out",
  http_server: true,
});

itest!(_033_import_map {
  args:
    "run --quiet --reload --importmap=importmaps/import_map.json --unstable importmaps/test.ts",
  output: "033_import_map.out",
});

itest!(import_map_no_unstable {
  args:
    "run --quiet --reload --importmap=importmaps/import_map.json importmaps/test.ts",
  output: "import_map_no_unstable.out",
  exit_code: 70,
});

itest!(_034_onload {
  args: "run --quiet --reload 034_onload/main.ts",
  output: "034_onload.out",
});

itest!(_035_cached_only_flag {
  args:
    "run --reload --cached-only http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "035_cached_only_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_036_import_map_fetch {
  args:
    "cache --quiet --reload --importmap=importmaps/import_map.json --unstable importmaps/test.ts",
  output: "036_import_map_fetch.out",
});

itest!(_037_fetch_multiple {
  args: "cache --reload fetch/test.ts fetch/other.ts",
  http_server: true,
  output: "037_fetch_multiple.out",
});

itest!(_038_checkjs {
  // checking if JS file is run through TS compiler
  args: "run --reload --config 038_checkjs.tsconfig.json 038_checkjs.js",
  exit_code: 1,
  output: "038_checkjs.js.out",
});

itest!(_041_dyn_import_eval {
  args: "eval import('./subdir/mod4.js').then(console.log)",
  output: "041_dyn_import_eval.out",
});

itest!(_041_info_flag {
  args: "info",
  output: "041_info_flag.out",
});

itest!(info_json {
  args: "info --json --unstable",
  output: "info_json.out",
});

itest!(_042_dyn_import_evalcontext {
  args: "run --quiet --allow-read --reload 042_dyn_import_evalcontext.ts",
  output: "042_dyn_import_evalcontext.ts.out",
});

itest!(_044_bad_resource {
  args: "run --quiet --reload --allow-read 044_bad_resource.ts",
  output: "044_bad_resource.ts.out",
  exit_code: 1,
});

itest!(_045_proxy {
  args: "run --allow-net --allow-env --allow-run --allow-read --reload --quiet 045_proxy_test.ts",
  output: "045_proxy_test.ts.out",
  http_server: true,
});

itest!(_046_tsx {
  args: "run --quiet --reload 046_jsx_test.tsx",
  output: "046_jsx_test.tsx.out",
});

itest!(_047_jsx {
  args: "run --quiet --reload 047_jsx_test.jsx",
  output: "047_jsx_test.jsx.out",
});

itest!(_048_media_types_jsx {
  args: "run  --reload 048_media_types_jsx.ts",
  output: "048_media_types_jsx.ts.out",
  http_server: true,
});

// TODO(nayeemrmn): This hits an SWC type-stripping bug:
// `error: Unterminated regexp literal at http://localhost:4545/cli/tests/subdir/mt_video_vdn_tsx.t2.tsx:4:19`
// Re-enable once fixed.
itest_ignore!(_049_info_flag_script_jsx {
  args: "info http://127.0.0.1:4545/cli/tests/048_media_types_jsx.ts",
  output: "049_info_flag_script_jsx.out",
  http_server: true,
});

itest!(_052_no_remote_flag {
  args:
    "run --reload --no-remote http://127.0.0.1:4545/cli/tests/019_media_types.ts",
  output: "052_no_remote_flag.out",
  exit_code: 1,
  http_server: true,
});

itest!(_054_info_local_imports {
  args: "info --quiet 005_more_imports.ts",
  output: "054_info_local_imports.out",
  exit_code: 0,
});

itest!(_055_info_file_json {
  args: "info --quiet --json --unstable 005_more_imports.ts",
  output: "055_info_file_json.out",
  exit_code: 0,
});

itest!(_056_make_temp_file_write_perm {
  args:
    "run --quiet --allow-read --allow-write=./subdir/ 056_make_temp_file_write_perm.ts",
  output: "056_make_temp_file_write_perm.out",
});

itest!(_058_tasks_microtasks_close {
  args: "run --quiet 058_tasks_microtasks_close.ts",
  output: "058_tasks_microtasks_close.ts.out",
});

itest!(_059_fs_relative_path_perm {
  args: "run 059_fs_relative_path_perm.ts",
  output: "059_fs_relative_path_perm.ts.out",
  exit_code: 1,
});

itest!(_060_deno_doc_displays_all_overloads_in_details_view {
  args: "doc 060_deno_doc_displays_all_overloads_in_details_view.ts NS.test",
  output: "060_deno_doc_displays_all_overloads_in_details_view.ts.out",
});

#[cfg(unix)]
#[test]
fn _061_permissions_request() {
  let args = "run --unstable 061_permissions_request.ts";
  let output = "061_permissions_request.ts.out";
  let input = b"g\nd\n";

  util::test_pty(args, output, input);
}

#[cfg(unix)]
#[test]
fn _062_permissions_request_global() {
  let args = "run --unstable 062_permissions_request_global.ts";
  let output = "062_permissions_request_global.ts.out";
  let input = b"g\n";

  util::test_pty(args, output, input);
}

itest!(_063_permissions_revoke {
  args: "run --unstable --allow-read=foo,bar 063_permissions_revoke.ts",
  output: "063_permissions_revoke.ts.out",
});

itest!(_064_permissions_revoke_global {
  args: "run --unstable --allow-read=foo,bar 064_permissions_revoke_global.ts",
  output: "064_permissions_revoke_global.ts.out",
});

itest!(js_import_detect {
  args: "run --quiet --reload js_import_detect.ts",
  output: "js_import_detect.ts.out",
  exit_code: 0,
});

itest!(lock_write_requires_lock {
  args: "run --lock-write some_file.ts",
  output: "lock_write_requires_lock.out",
  exit_code: 1,
});

itest!(lock_write_fetch {
  args:
    "run --quiet --allow-read --allow-write --allow-env --allow-run lock_write_fetch.ts",
  output: "lock_write_fetch.ts.out",
  exit_code: 0,
});

itest!(lock_check_ok {
  args: "run --lock=lock_check_ok.json http://127.0.0.1:4545/cli/tests/003_relative_import.ts",
  output: "003_relative_import.ts.out",
  http_server: true,
});

itest!(lock_check_ok2 {
  args: "run --lock=lock_check_ok2.json 019_media_types.ts",
  output: "019_media_types.ts.out",
  http_server: true,
});

itest!(lock_dynamic_imports {
  args: "run --lock=lock_dynamic_imports.json --allow-read --allow-net http://127.0.0.1:4545/cli/tests/013_dynamic_import.ts",
  output: "lock_dynamic_imports.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err {
  args: "run --lock=lock_check_err.json http://127.0.0.1:4545/cli/tests/003_relative_import.ts",
  output: "lock_check_err.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err2 {
  args: "run --lock=lock_check_err2.json 019_media_types.ts",
  output: "lock_check_err2.out",
  exit_code: 10,
  http_server: true,
});

itest!(lock_check_err_with_bundle {
  args: "bundle --lock=lock_check_err_with_bundle.json http://127.0.0.1:4545/cli/tests/subdir/mod1.ts",
  output: "lock_check_err_with_bundle.out",
  exit_code: 10,
  http_server: true,
});

itest!(async_error {
  exit_code: 1,
  args: "run --reload async_error.ts",
  output: "async_error.ts.out",
});

itest!(bundle {
  args: "bundle subdir/mod1.ts",
  output: "bundle.test.out",
});

itest!(fmt_check_tests_dir {
  args: "fmt --check ./",
  output: "fmt_check_tests_dir.out",
  exit_code: 1,
});

itest!(fmt_stdin {
  args: "fmt -",
  input: Some("const a = 1\n"),
  output_str: Some("const a = 1;\n"),
});

itest!(fmt_stdin_check_formatted {
  args: "fmt --check -",
  input: Some("const a = 1;\n"),
  output_str: Some(""),
});

itest!(fmt_stdin_check_not_formatted {
  args: "fmt --check -",
  input: Some("const a = 1\n"),
  output_str: Some("Not formatted stdin\n"),
});

itest!(circular1 {
  args: "run --reload circular1.js",
  output: "circular1.js.out",
});

itest!(config {
  args: "run --reload --config config.tsconfig.json config.ts",
  exit_code: 1,
  output: "config.ts.out",
});

itest!(error_001 {
  args: "run --reload error_001.ts",
  exit_code: 1,
  output: "error_001.ts.out",
});

itest!(error_002 {
  args: "run --reload error_002.ts",
  exit_code: 1,
  output: "error_002.ts.out",
});

itest!(error_003_typescript {
  args: "run --reload error_003_typescript.ts",
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

// Supposing that we've already attempted to run error_003_typescript.ts
// we want to make sure that JS wasn't emitted. Running again without reload flag
// should result in the same output.
// https://github.com/denoland/deno/issues/2436
itest!(error_003_typescript2 {
  args: "run error_003_typescript.ts",
  exit_code: 1,
  output: "error_003_typescript.ts.out",
});

itest!(error_004_missing_module {
  args: "run --reload error_004_missing_module.ts",
  exit_code: 1,
  output: "error_004_missing_module.ts.out",
});

itest!(error_005_missing_dynamic_import {
  args: "run --reload --allow-read --quiet error_005_missing_dynamic_import.ts",
  exit_code: 1,
  output: "error_005_missing_dynamic_import.ts.out",
});

itest!(error_006_import_ext_failure {
  args: "run --reload error_006_import_ext_failure.ts",
  exit_code: 1,
  output: "error_006_import_ext_failure.ts.out",
});

itest!(error_007_any {
  args: "run --reload error_007_any.ts",
  exit_code: 1,
  output: "error_007_any.ts.out",
});

itest!(error_008_checkjs {
  args: "run --reload error_008_checkjs.js",
  exit_code: 1,
  output: "error_008_checkjs.js.out",
});

itest!(error_011_bad_module_specifier {
  args: "run --reload error_011_bad_module_specifier.ts",
  exit_code: 1,
  output: "error_011_bad_module_specifier.ts.out",
});

itest!(error_012_bad_dynamic_import_specifier {
  args: "run --reload error_012_bad_dynamic_import_specifier.ts",
  exit_code: 1,
  output: "error_012_bad_dynamic_import_specifier.ts.out",
});

itest!(error_013_missing_script {
  args: "run --reload missing_file_name",
  exit_code: 1,
  output: "error_013_missing_script.out",
});

itest!(error_014_catch_dynamic_import_error {
  args: "run  --reload --allow-read error_014_catch_dynamic_import_error.js",
  output: "error_014_catch_dynamic_import_error.js.out",
});

itest!(error_015_dynamic_import_permissions {
  args: "run --reload --quiet error_015_dynamic_import_permissions.js",
  output: "error_015_dynamic_import_permissions.out",
  exit_code: 1,
  http_server: true,
});

// We have an allow-net flag but not allow-read, it should still result in error.
itest!(error_016_dynamic_import_permissions2 {
  args: "run --reload --allow-net error_016_dynamic_import_permissions2.js",
  output: "error_016_dynamic_import_permissions2.out",
  exit_code: 1,
  http_server: true,
});

itest!(error_017_hide_long_source_ts {
  args: "run --reload error_017_hide_long_source_ts.ts",
  output: "error_017_hide_long_source_ts.ts.out",
  exit_code: 1,
});

itest!(error_018_hide_long_source_js {
  args: "run error_018_hide_long_source_js.js",
  output: "error_018_hide_long_source_js.js.out",
  exit_code: 1,
});

itest!(error_019_stack_function {
  args: "run error_019_stack_function.ts",
  output: "error_019_stack_function.ts.out",
  exit_code: 1,
});

itest!(error_020_stack_constructor {
  args: "run error_020_stack_constructor.ts",
  output: "error_020_stack_constructor.ts.out",
  exit_code: 1,
});

itest!(error_021_stack_method {
  args: "run error_021_stack_method.ts",
  output: "error_021_stack_method.ts.out",
  exit_code: 1,
});

itest!(error_022_stack_custom_error {
  args: "run error_022_stack_custom_error.ts",
  output: "error_022_stack_custom_error.ts.out",
  exit_code: 1,
});

itest!(error_023_stack_async {
  args: "run error_023_stack_async.ts",
  output: "error_023_stack_async.ts.out",
  exit_code: 1,
});

itest!(error_024_stack_promise_all {
  args: "run error_024_stack_promise_all.ts",
  output: "error_024_stack_promise_all.ts.out",
  exit_code: 1,
});

itest!(error_025_tab_indent {
  args: "run error_025_tab_indent",
  output: "error_025_tab_indent.out",
  exit_code: 1,
});

itest!(error_no_check {
  args: "run --reload --no-check error_no_check.ts",
  output: "error_no_check.ts.out",
  exit_code: 1,
});

itest!(error_syntax {
  args: "run --reload error_syntax.js",
  exit_code: 1,
  output: "error_syntax.js.out",
});

itest!(error_syntax_empty_trailing_line {
  args: "run --reload error_syntax_empty_trailing_line.mjs",
  exit_code: 1,
  output: "error_syntax_empty_trailing_line.mjs.out",
});

itest!(error_type_definitions {
  args: "run --reload error_type_definitions.ts",
  exit_code: 1,
  output: "error_type_definitions.ts.out",
});

itest!(error_local_static_import_from_remote_ts {
  args: "run --reload http://localhost:4545/cli/tests/error_local_static_import_from_remote.ts",
  exit_code: 1,
  http_server: true,
  output: "error_local_static_import_from_remote.ts.out",
});

itest!(error_local_static_import_from_remote_js {
  args: "run --reload http://localhost:4545/cli/tests/error_local_static_import_from_remote.js",
  exit_code: 1,
  http_server: true,
  output: "error_local_static_import_from_remote.js.out",
});

itest!(exit_error42 {
  exit_code: 42,
  args: "run --quiet --reload exit_error42.ts",
  output: "exit_error42.ts.out",
});

itest!(https_import {
  args: "run --quiet --reload https_import.ts",
  output: "https_import.ts.out",
});

itest!(if_main {
  args: "run --quiet --reload if_main.ts",
  output: "if_main.ts.out",
});

itest!(import_meta {
  args: "run --quiet --reload import_meta.ts",
  output: "import_meta.ts.out",
});

itest!(main_module {
  args: "run --quiet --allow-read --reload main_module.ts",
  output: "main_module.ts.out",
});

itest!(no_check {
  args: "run --quiet --reload --no-check 006_url_imports.ts",
  output: "006_url_imports.ts.out",
  http_server: true,
});

itest!(lib_ref {
  args: "run --quiet --unstable --reload lib_ref.ts",
  output: "lib_ref.ts.out",
});

itest!(lib_runtime_api {
  args: "run --quiet --unstable --reload lib_runtime_api.ts",
  output: "lib_runtime_api.ts.out",
});

itest!(seed_random {
  args: "run --seed=100 seed_random.js",

  output: "seed_random.js.out",
});

itest!(type_definitions {
  args: "run --reload type_definitions.ts",
  output: "type_definitions.ts.out",
});

itest!(type_definitions_for_export {
  args: "run --reload type_definitions_for_export.ts",
  output: "type_definitions_for_export.ts.out",
  exit_code: 1,
});

itest!(type_directives_01 {
  args: "run --reload -L debug type_directives_01.ts",
  output: "type_directives_01.ts.out",
  http_server: true,
});

itest!(type_directives_02 {
  args: "run --reload -L debug type_directives_02.ts",
  output: "type_directives_02.ts.out",
});

itest!(type_directives_js_main {
  args: "run --reload -L debug type_directives_js_main.js",
  output: "type_directives_js_main.js.out",
  exit_code: 0,
});

itest!(type_directives_redirect {
  args: "run --reload type_directives_redirect.ts",
  output: "type_directives_redirect.ts.out",
  http_server: true,
});

itest!(type_headers_deno_types {
  args: "run --reload type_headers_deno_types.ts",
  output: "type_headers_deno_types.ts.out",
  http_server: true,
});

itest!(ts_type_imports {
  args: "run --reload ts_type_imports.ts",
  output: "ts_type_imports.ts.out",
  exit_code: 1,
});

itest!(ts_decorators {
  args: "run --reload -c tsconfig.decorators.json ts_decorators.ts",
  output: "ts_decorators.ts.out",
});

itest!(ts_type_only_import {
  args: "run --reload ts_type_only_import.ts",
  output: "ts_type_only_import.ts.out",
});

itest!(swc_syntax_error {
  args: "run --reload swc_syntax_error.ts",
  output: "swc_syntax_error.ts.out",
  exit_code: 1,
});

itest!(types {
  args: "types",
  output: "types.out",
});

itest!(unbuffered_stderr {
  args: "run --reload unbuffered_stderr.ts",
  output: "unbuffered_stderr.ts.out",
});

itest!(unbuffered_stdout {
  args: "run --quiet --reload unbuffered_stdout.ts",
  output: "unbuffered_stdout.ts.out",
});

// Cannot write the expression to evaluate as "console.log(typeof gc)"
// because itest! splits args on whitespace.
itest!(v8_flags_eval {
  args: "eval --v8-flags=--expose-gc console.log(typeof(gc))",
  output: "v8_flags.js.out",
});

itest!(v8_flags_run {
  args: "run --v8-flags=--expose-gc v8_flags.js",
  output: "v8_flags.js.out",
});

itest!(v8_flags_unrecognized {
  args: "repl --v8-flags=--foo,bar,--trace-gc,-baz",
  output: "v8_flags_unrecognized.out",
  exit_code: 1,
});

itest!(v8_help {
  args: "repl --v8-flags=--help",
  output: "v8_help.out",
});

itest!(unsupported_dynamic_import_scheme {
  args: "eval import('xxx:')",
  output: "unsupported_dynamic_import_scheme.out",
  exit_code: 1,
});

itest!(wasm {
  args: "run --quiet wasm.ts",
  output: "wasm.ts.out",
});

itest!(wasm_async {
  args: "run wasm_async.js",
  output: "wasm_async.out",
});

itest!(wasm_streaming {
  args: "run wasm_streaming.js",
  output: "wasm_streaming.out",
});

itest!(wasm_unreachable {
  args: "run wasm_unreachable.js",
  output: "wasm_unreachable.out",
  exit_code: 1,
});

itest!(top_level_await {
  args: "run --allow-read top_level_await.js",
  output: "top_level_await.out",
});

itest!(top_level_await_ts {
  args: "run --quiet --allow-read top_level_await.ts",
  output: "top_level_await.out",
});

itest!(top_level_for_await {
  args: "run --quiet top_level_for_await.js",
  output: "top_level_for_await.out",
});

itest!(top_level_for_await_ts {
  args: "run --quiet top_level_for_await.ts",
  output: "top_level_for_await.out",
});

itest!(unstable_disabled {
  args: "run --reload unstable.ts",
  exit_code: 1,
  output: "unstable_disabled.out",
});

itest!(unstable_enabled {
  args: "run --quiet --reload --unstable unstable.ts",
  output: "unstable_enabled.out",
});

itest!(unstable_disabled_js {
  args: "run --reload unstable.js",
  output: "unstable_disabled_js.out",
});

itest!(unstable_enabled_js {
  args: "run --quiet --reload --unstable unstable.ts",
  output: "unstable_enabled_js.out",
});

itest!(unstable_disabled_ts2551 {
  args: "run --reload unstable_ts2551.ts",
  exit_code: 1,
  output: "unstable_disabled_ts2551.out",
});

itest!(_053_import_compression {
  args: "run --quiet --reload --allow-net 053_import_compression/main.ts",
  output: "053_import_compression.out",
  http_server: true,
});

itest!(cafile_url_imports {
  args: "run --quiet --reload --cert tls/RootCA.pem cafile_url_imports.ts",
  output: "cafile_url_imports.ts.out",
  http_server: true,
});

itest!(cafile_ts_fetch {
  args:
    "run --quiet --reload --allow-net --cert tls/RootCA.pem cafile_ts_fetch.ts",
  output: "cafile_ts_fetch.ts.out",
  http_server: true,
});

itest!(cafile_eval {
  args: "eval --cert tls/RootCA.pem fetch('https://localhost:5545/cli/tests/cafile_ts_fetch.ts.out').then(r=>r.text()).then(t=>console.log(t.trimEnd()))",
  output: "cafile_ts_fetch.ts.out",
  http_server: true,
});

itest!(cafile_info {
  args:
    "info --quiet --cert tls/RootCA.pem https://localhost:5545/cli/tests/cafile_info.ts",
  output: "cafile_info.ts.out",
  http_server: true,
});

itest!(disallow_http_from_https_js {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/cli/tests/disallow_http_from_https.js",
  output: "disallow_http_from_https_js.out",
  http_server: true,
  exit_code: 1,
});

itest!(disallow_http_from_https_ts {
  args: "run --quiet --reload --cert tls/RootCA.pem https://localhost:5545/cli/tests/disallow_http_from_https.ts",
  output: "disallow_http_from_https_ts.out",
  http_server: true,
  exit_code: 1,
});

itest!(tsx_imports {
  args: "run --reload tsx_imports.ts",
  output: "tsx_imports.ts.out",
});

itest!(fix_js_import_js {
  args: "run --quiet --reload fix_js_import_js.ts",
  output: "fix_js_import_js.ts.out",
});

itest!(fix_js_imports {
  args: "run --quiet --reload fix_js_imports.ts",
  output: "fix_js_imports.ts.out",
});

itest!(es_private_fields {
  args: "run --quiet --reload es_private_fields.js",
  output: "es_private_fields.js.out",
});

itest!(cjs_imports {
  args: "run --quiet --reload cjs_imports.ts",
  output: "cjs_imports.ts.out",
});

itest!(ts_import_from_js {
  args: "run --quiet --reload ts_import_from_js.js",
  output: "ts_import_from_js.js.out",
  http_server: true,
});

itest!(jsx_import_from_ts {
  args: "run --quiet --reload jsx_import_from_ts.ts",
  output: "jsx_import_from_ts.ts.out",
});

itest!(single_compile_with_reload {
  args: "run --reload --allow-read single_compile_with_reload.ts",
  output: "single_compile_with_reload.ts.out",
});

itest!(performance_stats {
  args: "cache --reload --log-level debug 002_hello.ts",
  output: "performance_stats.out",
});

itest!(proto_exploit {
  args: "run proto_exploit.js",
  output: "proto_exploit.js.out",
});

itest!(deno_lint {
  args: "lint --unstable lint/file1.js lint/file2.ts lint/ignored_file.ts",
  output: "lint/expected.out",
  exit_code: 1,
});

itest!(deno_lint_json {
  args:
    "lint --unstable --json lint/file1.js lint/file2.ts lint/ignored_file.ts lint/malformed.js",
  output: "lint/expected_json.out",
  exit_code: 1,
});

itest!(deno_lint_ignore {
  args: "lint --unstable --ignore=lint/file1.js,lint/malformed.js lint/",
  output: "lint/expected_ignore.out",
  exit_code: 1,
});

itest!(deno_lint_glob {
  args: "lint --unstable --ignore=lint/malformed.js lint/",
  output: "lint/expected_glob.out",
  exit_code: 1,
});

itest!(deno_doc_builtin {
  args: "doc",
  output: "deno_doc_builtin.out",
});

itest!(deno_doc {
  args: "doc deno_doc.ts",
  output: "deno_doc.out",
});

itest!(compiler_js_error {
  args: "run --unstable compiler_js_error.ts",
  output: "compiler_js_error.ts.out",
  exit_code: 1,
});

itest!(import_file_with_colon {
  args: "run --quiet --reload import_file_with_colon.ts",
  output: "import_file_with_colon.ts.out",
  http_server: true,
});

#[test]
fn cafile_env_fetch() {
  use url::Url;
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    Url::parse("https://localhost:5545/cli/tests/cafile_url_imports.ts")
      .unwrap();
  let cafile = util::root_path().join("cli/tests/tls/RootCA.pem");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .env("DENO_CERT", cafile)
    .current_dir(util::root_path())
    .arg("cache")
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
}

#[test]
fn cafile_fetch() {
  use url::Url;
  let _g = util::http_server();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let module_url =
    Url::parse("http://localhost:4545/cli/tests/cafile_url_imports.ts")
      .unwrap();
  let cafile = util::root_path().join("cli/tests/tls/RootCA.pem");
  let output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::root_path())
    .arg("cache")
    .arg("--cert")
    .arg(cafile)
    .arg(module_url.to_string())
    .output()
    .expect("Failed to spawn script");
  assert!(output.status.success());
  let out = std::str::from_utf8(&output.stdout).unwrap();
  assert_eq!(out, "");
}

#[test]
fn cafile_install_remote_module() {
  let _g = util::http_server();
  let temp_dir = TempDir::new().expect("tempdir fail");
  let bin_dir = temp_dir.path().join("bin");
  std::fs::create_dir(&bin_dir).unwrap();
  let deno_dir = TempDir::new().expect("tempdir fail");
  let cafile = util::root_path().join("cli/tests/tls/RootCA.pem");

  let install_output = Command::new(util::deno_exe_path())
    .env("DENO_DIR", deno_dir.path())
    .current_dir(util::root_path())
    .arg("install")
    .arg("--cert")
    .arg(cafile)
    .arg("--root")
    .arg(temp_dir.path())
    .arg("-n")
    .arg("echo_test")
    .arg("https://localhost:5545/cli/tests/echo.ts")
    .output()
    .expect("Failed to spawn script");
  assert!(install_output.status.success());

  let mut echo_test_path = bin_dir.join("echo_test");
  if cfg!(windows) {
    echo_test_path = echo_test_path.with_extension("cmd");
  }
  assert!(echo_test_path.exists());

  let output = Command::new(echo_test_path)
    .current_dir(temp_dir.path())
    .arg("foo")
    .env("PATH", util::target_dir())
    .output()
    .expect("failed to spawn script");
  let stdout = std::str::from_utf8(&output.stdout).unwrap().trim();
  assert!(stdout.ends_with("foo"));
}

#[test]
fn cafile_bundle_remote_exports() {
  let _g = util::http_server();

  // First we have to generate a bundle of some remote module that has exports.
  let mod1 = "https://localhost:5545/cli/tests/subdir/mod1.ts";
  let cafile = util::root_path().join("cli/tests/tls/RootCA.pem");
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod1.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("bundle")
    .arg("--cert")
    .arg(cafile)
    .arg(mod1)
    .arg(&bundle)
    .spawn()
    .expect("failed to spawn script");
  let status = deno.wait().expect("failed to wait for the child process");
  assert!(status.success());
  assert!(bundle.is_file());

  // Now we try to use that bundle from another module.
  let test = t.path().join("test.js");
  std::fs::write(
    &test,
    "
      import { printHello3 } from \"./mod1.bundle.js\";
      printHello3(); ",
  )
  .expect("error writing file");

  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg(&test)
    .output()
    .expect("failed to spawn script");
  // check the output of the test.ts program.
  assert!(std::str::from_utf8(&output.stdout)
    .unwrap()
    .trim()
    .ends_with("Hello"));
  assert_eq!(output.stderr, b"");
}

#[test]
fn test_permissions_with_allow() {
  for permission in &util::PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!("--allow-{0}", permission))
      .arg("permission_test.ts")
      .arg(format!("{0}Required", permission))
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_without_allow() {
  for permission in &util::PERMISSION_VARIANTS {
    let (_, err) = util::run_and_collect_output(
      false,
      &format!("run permission_test.ts {0}Required", permission),
      None,
      None,
      false,
    );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }
}

#[test]
fn test_permissions_rw_inside_project_dir() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  for permission in &PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!(
        "--allow-{0}={1}",
        permission,
        util::root_path().into_os_string().into_string().unwrap()
      ))
      .arg("complex_permissions_test.ts")
      .arg(permission)
      .arg("complex_permissions_test.ts")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_rw_outside_test_dir() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  for permission in &PERMISSION_VARIANTS {
    let (_, err) = util::run_and_collect_output(
      false,
      &format!(
        "run --allow-{0}={1} complex_permissions_test.ts {0} {2}",
        permission,
        util::root_path()
          .join("cli")
          .join("tests")
          .into_os_string()
          .into_string()
          .unwrap(),
        util::root_path()
          .join("Cargo.toml")
          .into_os_string()
          .into_string()
          .unwrap(),
      ),
      None,
      None,
      false,
    );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }
}

#[test]
fn test_permissions_rw_inside_test_dir() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  for permission in &PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!(
        "--allow-{0}={1}",
        permission,
        util::root_path()
          .join("cli")
          .join("tests")
          .into_os_string()
          .into_string()
          .unwrap()
      ))
      .arg("complex_permissions_test.ts")
      .arg(permission)
      .arg("complex_permissions_test.ts")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_rw_outside_test_and_js_dir() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  let test_dir = util::root_path()
    .join("cli")
    .join("tests")
    .into_os_string()
    .into_string()
    .unwrap();
  let js_dir = util::root_path()
    .join("js")
    .into_os_string()
    .into_string()
    .unwrap();
  for permission in &PERMISSION_VARIANTS {
    let (_, err) = util::run_and_collect_output(
      false,
      &format!(
        "run --allow-{0}={1},{2} complex_permissions_test.ts {0} {3}",
        permission,
        test_dir,
        js_dir,
        util::root_path()
          .join("Cargo.toml")
          .into_os_string()
          .into_string()
          .unwrap(),
      ),
      None,
      None,
      false,
    );
    assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
  }
}

#[test]
fn test_permissions_rw_inside_test_and_js_dir() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  let test_dir = util::root_path()
    .join("cli")
    .join("tests")
    .into_os_string()
    .into_string()
    .unwrap();
  let js_dir = util::root_path()
    .join("js")
    .into_os_string()
    .into_string()
    .unwrap();
  for permission in &PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!("--allow-{0}={1},{2}", permission, test_dir, js_dir))
      .arg("complex_permissions_test.ts")
      .arg(permission)
      .arg("complex_permissions_test.ts")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_rw_relative() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  for permission in &PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!("--allow-{0}=.", permission))
      .arg("complex_permissions_test.ts")
      .arg(permission)
      .arg("complex_permissions_test.ts")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_rw_no_prefix() {
  const PERMISSION_VARIANTS: [&str; 2] = ["read", "write"];
  for permission in &PERMISSION_VARIANTS {
    let status = util::deno_cmd()
      .current_dir(&util::tests_path())
      .arg("run")
      .arg(format!("--allow-{0}=tls/../", permission))
      .arg("complex_permissions_test.ts")
      .arg(permission)
      .arg("complex_permissions_test.ts")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }
}

#[test]
fn test_permissions_net_fetch_allow_localhost_4545() {
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=localhost:4545 complex_permissions_test.ts netFetch http://localhost:4545/",
			None,
      None,
      true,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_fetch_allow_deno_land() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=deno.land complex_permissions_test.ts netFetch http://localhost:4545/",
			None,
			None,
			true,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_fetch_localhost_4545_fail() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=localhost:4545 complex_permissions_test.ts netFetch http://localhost:4546/",
			None,
			None,
			true,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_fetch_localhost() {
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=localhost complex_permissions_test.ts netFetch http://localhost:4545/ http://localhost:4546/ http://localhost:4547/",
			None,
			None,
			true,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_connect_allow_localhost_ip_4555() {
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=127.0.0.1:4545 complex_permissions_test.ts netConnect 127.0.0.1:4545",
			None,
			None,
			true,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_connect_allow_deno_land() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=deno.land complex_permissions_test.ts netConnect 127.0.0.1:4546",
			None,
			None,
			true,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_connect_allow_localhost_ip_4545_fail() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=127.0.0.1:4545 complex_permissions_test.ts netConnect 127.0.0.1:4546",
			None,
			None,
			true,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_connect_allow_localhost_ip() {
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=127.0.0.1 complex_permissions_test.ts netConnect 127.0.0.1:4545 127.0.0.1:4546 127.0.0.1:4547",
			None,
			None,
			true,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_listen_allow_localhost_4555() {
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=localhost:4558 complex_permissions_test.ts netListen localhost:4558",
			None,
			None,
			false,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_listen_allow_deno_land() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=deno.land complex_permissions_test.ts netListen localhost:4545",
			None,
			None,
			false,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_listen_allow_localhost_4555_fail() {
  let (_, err) = util::run_and_collect_output(
    false,
			"run --allow-net=localhost:4555 complex_permissions_test.ts netListen localhost:4556",
			None,
			None,
			false,
		);
  assert!(err.contains(util::PERMISSION_DENIED_PATTERN));
}

#[test]
fn test_permissions_net_listen_allow_localhost() {
  // Port 4600 is chosen to not colide with those used by
  // target/debug/test_server
  let (_, err) = util::run_and_collect_output(
    true,
			"run --allow-net=localhost complex_permissions_test.ts netListen localhost:4600",
			None,
			None,
      false,
		);
  assert!(!err.contains(util::PERMISSION_DENIED_PATTERN));
}

fn inspect_flag_with_unique_port(flag_prefix: &str) -> String {
  use std::sync::atomic::{AtomicU16, Ordering};
  static PORT: AtomicU16 = AtomicU16::new(9229);
  let port = PORT.fetch_add(1, Ordering::Relaxed);
  format!("{}=127.0.0.1:{}", flag_prefix, port)
}

fn extract_ws_url_from_stderr(
  stderr_lines: &mut impl std::iter::Iterator<Item = String>,
) -> url::Url {
  let stderr_first_line = stderr_lines.next().unwrap();
  assert!(stderr_first_line.starts_with("Debugger listening on "));
  let v: Vec<_> = stderr_first_line.match_indices("ws:").collect();
  assert_eq!(v.len(), 1);
  let ws_url_index = v[0].0;
  let ws_url = &stderr_first_line[ws_url_index..];
  url::Url::parse(ws_url).unwrap()
}

#[tokio::test]
async fn inspector_connect() {
  let script = util::tests_path().join("inspector1.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect"))
    .arg(script)
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  // We use tokio_tungstenite as a websocket client because warp (which is
  // a dependency of Deno) uses it.
  let (_socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!("101 Switching Protocols", response.status().to_string());
  child.kill().unwrap();
  child.wait().unwrap();
}

enum TestStep {
  StdOut(&'static str),
  StdErr(&'static str),
  WsRecv(&'static str),
  WsSend(&'static str),
}

#[tokio::test]
async fn inspector_break_on_first_line() {
  let script = util::tests_path().join("inspector2.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.paused","#),
    WsSend(
      r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"Deno.core.print(\"hello from the inspector\\n\")","contextId":1,"includeCommandLineAPI":true,"silent":false,"returnByValue":true}}"#,
    ),
    WsRecv(r#"{"id":4,"result":{"result":{"type":"undefined"}}}"#),
    StdOut("hello from the inspector"),
    WsSend(r#"{"id":5,"method":"Debugger.resume"}"#),
    WsRecv(r#"{"id":5,"result":{}}"#),
    StdOut("hello from the script"),
  ];

  for step in test_steps {
    match step {
      StdOut(s) => assert_eq!(&stdout_lines.next().unwrap(), s),
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
      _ => unreachable!(),
    }
  }

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_pause() {
  let script = util::tests_path().join("inspector1.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect"))
    .arg(script)
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  // We use tokio_tungstenite as a websocket client because warp (which is
  // a dependency of Deno) uses it.
  let (mut socket, _) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");

  /// Returns the next websocket message as a string ignoring
  /// Debugger.scriptParsed messages.
  async fn ws_read_msg(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
  ) -> String {
    use futures::stream::StreamExt;
    while let Some(msg) = socket.next().await {
      let msg = msg.unwrap().to_string();
      // FIXME(bartlomieju): fails because there's a file loaded
      // called 150_errors.js
      // assert!(!msg.contains("error"));
      if !msg.contains("Debugger.scriptParsed") {
        return msg;
      }
    }
    unreachable!()
  }

  socket
    .send(r#"{"id":6,"method":"Debugger.enable"}"#.into())
    .await
    .unwrap();

  let msg = ws_read_msg(&mut socket).await;
  println!("response msg 1 {}", msg);
  assert!(msg.starts_with(r#"{"id":6,"result":{"debuggerId":"#));

  socket
    .send(r#"{"id":31,"method":"Debugger.pause"}"#.into())
    .await
    .unwrap();

  let msg = ws_read_msg(&mut socket).await;
  println!("response msg 2 {}", msg);
  assert_eq!(msg, r#"{"id":31,"result":{}}"#);

  child.kill().unwrap();
}

#[tokio::test]
async fn inspector_port_collision() {
  // Skip this test on WSL, which allows multiple processes to listen on the
  // same port, rather than making `bind()` fail with `EADDRINUSE`.
  if cfg!(target_os = "linux") && std::env::var_os("WSL_DISTRO_NAME").is_some()
  {
    return;
  }

  let script = util::tests_path().join("inspector1.js");
  let inspect_flag = inspect_flag_with_unique_port("--inspect");

  let mut child1 = util::deno_cmd()
    .arg("run")
    .arg(&inspect_flag)
    .arg(script.clone())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr_1 = child1.stderr.as_mut().unwrap();
  let mut stderr_1_lines = std::io::BufReader::new(stderr_1)
    .lines()
    .map(|r| r.unwrap());
  let _ = extract_ws_url_from_stderr(&mut stderr_1_lines);

  let mut child2 = util::deno_cmd()
    .arg("run")
    .arg(&inspect_flag)
    .arg(script)
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr_2 = child2.stderr.as_mut().unwrap();
  let stderr_2_error_message = std::io::BufReader::new(stderr_2)
    .lines()
    .map(|r| r.unwrap())
    .inspect(|line| assert!(!line.contains("Debugger listening")))
    .find(|line| line.contains("Cannot start inspector server"));
  assert!(stderr_2_error_message.is_some());

  child1.kill().unwrap();
  child1.wait().unwrap();
  child2.wait().unwrap();
}

#[tokio::test]
async fn inspector_does_not_hang() {
  let script = util::tests_path().join("inspector3.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect-brk"))
    .env("NO_COLOR", "1")
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.paused","#),
    WsSend(r#"{"id":4,"method":"Debugger.resume"}"#),
    WsRecv(r#"{"id":4,"result":{}}"#),
    WsRecv(r#"{"method":"Debugger.resumed","params":{}}"#),
  ];

  for step in test_steps {
    match step {
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
      _ => unreachable!(),
    }
  }

  for i in 0..128u32 {
    let request_id = i + 10;
    // Expect the number {i} on stdout.
    let s = format!("{}", i);
    assert_eq!(stdout_lines.next().unwrap(), s);
    // Expect hitting the `debugger` statement.
    let s = r#"{"method":"Debugger.paused","#;
    assert!(socket_rx.next().await.unwrap().starts_with(s));
    // Send the 'Debugger.resume' request.
    let s = format!(r#"{{"id":{},"method":"Debugger.resume"}}"#, request_id);
    socket_tx.send(s.into()).await.unwrap();
    // Expect confirmation of the 'Debugger.resume' request.
    let s = format!(r#"{{"id":{},"result":{{}}}}"#, request_id);
    assert_eq!(socket_rx.next().await.unwrap(), s);
    let s = r#"{"method":"Debugger.resumed","params":{}}"#;
    assert_eq!(socket_rx.next().await.unwrap(), s);
  }

  // Check that we can gracefully close the websocket connection.
  socket_tx.close().await.unwrap();
  socket_rx.for_each(|_| async {}).await;

  assert_eq!(&stdout_lines.next().unwrap(), "done");
  assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn inspector_without_brk_runs_code() {
  let script = util::tests_path().join("inspector4.js");
  let mut child = util::deno_cmd()
    .arg("run")
    .arg(inspect_flag_with_unique_port("--inspect"))
    .arg(script)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());
  let _ = extract_ws_url_from_stderr(&mut stderr_lines);

  // Check that inspector actually runs code without waiting for inspector
  // connection.
  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stdout_first_line = stdout_lines.next().unwrap();
  assert_eq!(stdout_first_line, "hello");

  child.kill().unwrap();
  child.wait().unwrap();
}

#[tokio::test]
async fn inspector_runtime_evaluate_does_not_crash() {
  let mut child = util::deno_cmd()
    .arg("repl")
    .arg(inspect_flag_with_unique_port("--inspect"))
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines = std::io::BufReader::new(stderr)
    .lines()
    .map(|r| r.unwrap())
    .filter(|s| s.as_str() != "Debugger session started.");
  let ws_url = extract_ws_url_from_stderr(&mut stderr_lines);

  let (socket, response) = tokio_tungstenite::connect_async(ws_url)
    .await
    .expect("Can't connect");
  assert_eq!(response.status(), 101); // Switching protocols.

  let (mut socket_tx, socket_rx) = socket.split();
  let mut socket_rx =
    socket_rx.map(|msg| msg.unwrap().to_string()).filter(|msg| {
      let pass = !msg.starts_with(r#"{"method":"Debugger.scriptParsed","#);
      futures::future::ready(pass)
    });

  let stdin = child.stdin.take().unwrap();

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines = std::io::BufReader::new(stdout)
    .lines()
    .map(|r| r.unwrap())
    .filter(|s| !s.starts_with("Deno "));

  use TestStep::*;
  let test_steps = vec![
    WsSend(r#"{"id":1,"method":"Runtime.enable"}"#),
    WsSend(r#"{"id":2,"method":"Debugger.enable"}"#),
    WsRecv(
      r#"{"method":"Runtime.executionContextCreated","params":{"context":{"id":1,"#,
    ),
    WsRecv(r#"{"id":1,"result":{}}"#),
    WsRecv(r#"{"id":2,"result":{"debuggerId":"#),
    WsSend(r#"{"id":3,"method":"Runtime.runIfWaitingForDebugger"}"#),
    WsRecv(r#"{"id":3,"result":{}}"#),
    StdOut("exit using ctrl+d or close()"),
    WsSend(
      r#"{"id":4,"method":"Runtime.compileScript","params":{"expression":"Deno.cwd()","sourceURL":"","persistScript":false,"executionContextId":1}}"#,
    ),
    WsRecv(r#"{"id":4,"result":{}}"#),
    WsSend(
      r#"{"id":5,"method":"Runtime.evaluate","params":{"expression":"Deno.cwd()","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ),
    WsRecv(r#"{"id":5,"result":{"result":{"type":"string","value":""#),
    WsSend(
      r#"{"id":6,"method":"Runtime.evaluate","params":{"expression":"console.error('done');","objectGroup":"console","includeCommandLineAPI":true,"silent":false,"contextId":1,"returnByValue":true,"generatePreview":true,"userGesture":true,"awaitPromise":false,"replMode":true}}"#,
    ),
    WsRecv(r#"{"id":6,"result":{"result":{"type":"undefined"}}}"#),
    StdErr("done"),
  ];

  for step in test_steps {
    match step {
      StdOut(s) => assert_eq!(&stdout_lines.next().unwrap(), s),
      StdErr(s) => assert_eq!(&stderr_lines.next().unwrap(), s),
      WsRecv(s) => assert!(socket_rx.next().await.unwrap().starts_with(s)),
      WsSend(s) => socket_tx.send(s.into()).await.unwrap(),
    }
  }

  drop(stdin);
  child.wait().unwrap();
}

#[test]
fn exec_path() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("--allow-read")
    .arg("cli/tests/exec_path.ts")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
  let stdout_str = std::str::from_utf8(&output.stdout).unwrap().trim();
  let actual =
    std::fs::canonicalize(&std::path::Path::new(stdout_str)).unwrap();
  let expected = std::fs::canonicalize(util::deno_exe_path()).unwrap();
  assert_eq!(expected, actual);
}

#[cfg(not(windows))]
#[test]
fn set_raw_should_not_panic_on_no_tty() {
  let output = util::deno_cmd()
    .arg("eval")
    .arg("--unstable")
    .arg("Deno.setRaw(Deno.stdin.rid, true)")
    // stdin set to piped so it certainly does not refer to TTY
    .stdin(std::process::Stdio::piped())
    // stderr is piped so we can capture output.
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(!output.status.success());
  let stderr = std::str::from_utf8(&output.stderr).unwrap().trim();
  assert!(stderr.contains("BadResource"));
}

#[cfg(windows)]
// Clippy suggests to remove the `NoStd` prefix from all variants. I disagree.
#[allow(clippy::enum_variant_names)]
enum WinProcConstraints {
  NoStdIn,
  NoStdOut,
  NoStdErr,
}

#[cfg(windows)]
fn run_deno_script_constrained(
  script_path: std::path::PathBuf,
  constraints: WinProcConstraints,
) -> Result<(), i64> {
  let file_path = "cli/tests/DenoWinRunner.ps1";
  let constraints = match constraints {
    WinProcConstraints::NoStdIn => "1",
    WinProcConstraints::NoStdOut => "2",
    WinProcConstraints::NoStdErr => "4",
  };
  let deno_exe_path = util::deno_exe_path()
    .into_os_string()
    .into_string()
    .unwrap();

  let deno_script_path = script_path.into_os_string().into_string().unwrap();

  let args = vec![&deno_exe_path[..], &deno_script_path[..], constraints];
  util::run_powershell_script_file(file_path, args)
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdin() {
  let output = run_deno_script_constrained(
    util::tests_path().join("echo.ts"),
    WinProcConstraints::NoStdIn,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stdout() {
  let output = run_deno_script_constrained(
    util::tests_path().join("echo.ts"),
    WinProcConstraints::NoStdOut,
  );
  output.unwrap();
}

#[cfg(windows)]
#[test]
fn should_not_panic_on_no_stderr() {
  let output = run_deno_script_constrained(
    util::tests_path().join("echo.ts"),
    WinProcConstraints::NoStdErr,
  );
  output.unwrap();
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_home_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("cli/tests/echo.ts")
    .env_remove("HOME")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[test]
fn should_not_panic_on_undefined_deno_dir_environment_variable() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("cli/tests/echo.ts")
    .env_remove("DENO_DIR")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}

#[cfg(not(windows))]
#[test]
fn should_not_panic_on_undefined_deno_dir_and_home_environment_variables() {
  let output = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("run")
    .arg("cli/tests/echo.ts")
    .env_remove("DENO_DIR")
    .env_remove("HOME")
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap()
    .wait_with_output()
    .unwrap();
  assert!(output.status.success());
}
