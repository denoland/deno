// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate lazy_static;

use os_pipe::pipe;
use regex::Regex;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tempfile::TempDir;

pub const PERMISSION_VARIANTS: [&str; 5] =
  ["read", "write", "env", "net", "run"];
pub const PERMISSION_DENIED_PATTERN: &str = "PermissionDenied";

lazy_static! {
  static ref DENO_DIR: TempDir = TempDir::new().expect("tempdir fail");

  // STRIP_ANSI_RE and strip_ansi_codes are lifted from the "console" crate.
  // Copyright 2017 Armin Ronacher <armin.ronacher@active-4.com>. MIT License.
  static ref STRIP_ANSI_RE: Regex = Regex::new(
          r"[\x1b\x9b][\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
  ).unwrap();

  static ref SERVER: Mutex<Option<Child>> = Mutex::new(None);
  static ref SERVER_COUNT: AtomicUsize = AtomicUsize::new(0);
}

pub fn root_path() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

pub fn tests_path() -> PathBuf {
  root_path().join("cli").join("tests")
}

pub fn target_dir() -> PathBuf {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  println!("target_dir {}", target_dir.display());
  target_dir.into()
}

pub fn deno_exe_path() -> PathBuf {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

pub struct HttpServerGuard {}

impl Drop for HttpServerGuard {
  fn drop(&mut self) {
    let count = SERVER_COUNT.fetch_sub(1, Ordering::SeqCst);
    // If no more tests hold guard we can kill the server

    if count == 1 {
      kill_http_server();
    }
  }
}

fn kill_http_server() {
  let mut server_guard = SERVER.lock().unwrap();
  let mut child = server_guard
    .take()
    .expect("Trying to kill server but already killed");
  match child.try_wait() {
    Ok(None) => {
      child.kill().expect("failed to kill http_server.py");
    }
    Ok(Some(status)) => panic!("http_server.py exited unexpectedly {}", status),
    Err(e) => panic!("http_server.py error: {}", e),
  }
  drop(server_guard);
}

/// Starts tools/http_server.py when the returned guard is dropped and there are
// no more guard being held, the server will be killed.
pub fn http_server() -> HttpServerGuard {
  SERVER_COUNT.fetch_add(1, Ordering::SeqCst);

  {
    let mut server_guard = SERVER.lock().unwrap();
    if server_guard.is_none() {
      println!("tools/http_server.py starting...");
      let mut child = Command::new("python")
        .current_dir(root_path())
        .args(&["-u", "tools/http_server.py"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute child");

      let stdout = child.stdout.as_mut().unwrap();
      use std::io::{BufRead, BufReader};
      let lines = BufReader::new(stdout).lines();
      // Wait for "ready" on stdout. See tools/http_server.py
      for maybe_line in lines {
        if let Ok(line) = maybe_line {
          if line.starts_with("ready") {
            break;
          }
        } else {
          panic!(maybe_line.unwrap_err());
        }
      }
      server_guard.replace(child);
    }
  }

  HttpServerGuard {}
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<str> {
  STRIP_ANSI_RE.replace_all(s, "")
}

pub fn run_and_collect_output(
  expect_success: bool,
  args: &str,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  let mut deno_process_builder = deno_cmd();
  deno_process_builder
    .args(args.split_whitespace())
    .current_dir(&tests_path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(envs) = envs {
    deno_process_builder.envs(envs);
  }
  let http_guard = if need_http_server {
    Some(http_server())
  } else {
    None
  };
  let mut deno = deno_process_builder
    .spawn()
    .expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = deno.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = deno.wait_with_output().expect("failed to wait on child");
  drop(http_guard);
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{}>>>", stdout);
    eprintln!("stderr: <<<{}>>>", stderr);
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn deno_cmd() -> Command {
  let e = deno_exe_path();
  assert!(e.exists());
  let mut c = Command::new(e);
  c.env("DENO_DIR", DENO_DIR.path());
  c
}

pub fn run_python_script(script: &str) {
  let output = Command::new("python")
    .env("DENO_DIR", DENO_DIR.path())
    .current_dir(root_path())
    .arg(script)
    .arg(format!("--build-dir={}", target_dir().display()))
    .arg(format!("--executable={}", deno_exe_path().display()))
    .output()
    .expect("failed to spawn script");
  if !output.status.success() {
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    panic!(
      "{} executed with failing error code\n{}{}",
      script, stdout, stderr
    );
  }
}

#[derive(Debug, Default)]
pub struct CheckOutputIntegrationTest {
  pub args: &'static str,
  pub output: &'static str,
  pub input: Option<&'static str>,
  pub output_str: Option<&'static str>,
  pub exit_code: i32,
  pub http_server: bool,
}

impl CheckOutputIntegrationTest {
  pub fn run(&self) {
    let args = self.args.split_whitespace();
    let root = root_path();
    let deno_exe = deno_exe_path();
    println!("root path {}", root.display());
    println!("deno_exe path {}", deno_exe.display());

    let http_server_guard = if self.http_server {
      Some(http_server())
    } else {
      None
    };

    let (mut reader, writer) = pipe().unwrap();
    let tests_dir = root.join("cli").join("tests");
    let mut command = deno_cmd();
    println!("deno_exe args {}", self.args);
    println!("deno_exe tests path {:?}", &tests_dir);
    command.args(args);
    command.current_dir(&tests_dir);
    command.stdin(Stdio::piped());
    let writer_clone = writer.try_clone().unwrap();
    command.stderr(writer_clone);
    command.stdout(writer);

    let mut process = command.spawn().expect("failed to execute process");

    if let Some(input) = self.input {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{}", input).unwrap();
    }

    // Very important when using pipes: This parent process is still
    // holding its copies of the write ends, and we have to close them
    // before we read, otherwise the read end will never report EOF. The
    // Command object owns the writers now, and dropping it closes them.
    drop(command);

    let mut actual = String::new();
    reader.read_to_string(&mut actual).unwrap();

    let status = process.wait().expect("failed to finish process");
    let exit_code = status.code().unwrap();

    drop(http_server_guard);

    actual = strip_ansi_codes(&actual).to_string();

    if self.exit_code != exit_code {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      panic!(
        "bad exit code, expected: {:?}, actual: {:?}",
        self.exit_code, exit_code
      );
    }

    let expected = if let Some(s) = self.output_str {
      s.to_owned()
    } else {
      let output_path = tests_dir.join(self.output);
      println!("output path {}", output_path.display());
      std::fs::read_to_string(output_path).expect("cannot read output")
    };

    if !wildcard_match(&expected, &actual) {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      println!("EXPECTED\n{}\nEXPECTED", expected);
      panic!("pattern match failed");
    }
  }
}

fn wildcard_match(pattern: &str, s: &str) -> bool {
  pattern_match(pattern, s, "[WILDCARD]")
}

pub fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
  // Normalize line endings
  let mut s = s.replace("\r\n", "\n");
  let pattern = pattern.replace("\r\n", "\n");

  if pattern == wildcard {
    return true;
  }

  let parts = pattern.split(wildcard).collect::<Vec<&str>>();
  if parts.len() == 1 {
    return pattern == s;
  }

  if !s.starts_with(parts[0]) {
    return false;
  }

  // If the first line of the pattern is just a wildcard the newline character
  // needs to be pre-pended so it can safely match anything or nothing and
  // continue matching.
  if pattern.lines().next() == Some(wildcard) {
    s.insert_str(0, "\n");
  }

  let mut t = s.split_at(parts[0].len());

  for (i, part) in parts.iter().enumerate() {
    if i == 0 {
      continue;
    }
    dbg!(part, i);
    if i == parts.len() - 1 && (*part == "" || *part == "\n") {
      dbg!("exit 1 true", i);
      return true;
    }
    if let Some(found) = t.1.find(*part) {
      dbg!("found ", found);
      t = t.1.split_at(found + part.len());
    } else {
      dbg!("exit false ", i);
      return false;
    }
  }

  dbg!("end ", t.1.len());
  t.1.is_empty()
}

#[test]
fn test_wildcard_match() {
  let fixtures = vec![
    ("foobarbaz", "foobarbaz", true),
    ("[WILDCARD]", "foobarbaz", true),
    ("foobar", "foobarbaz", false),
    ("foo[WILDCARD]baz", "foobarbaz", true),
    ("foo[WILDCARD]baz", "foobazbar", false),
    ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", true),
    ("foo[WILDCARD]", "foobar", true),
    ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", true),
    // check with different line endings
    ("foo[WILDCARD]\nbaz[WILDCARD]\n", "foobar\nbazqat\n", true),
    (
      "foo[WILDCARD]\nbaz[WILDCARD]\n",
      "foobar\r\nbazqat\r\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\n",
      "foobar\nbazqat\r\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
      "foobar\nbazqat\n",
      true,
    ),
    (
      "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
      "foobar\r\nbazqat\r\n",
      true,
    ),
  ];

  // Iterate through the fixture lists, testing each one
  for (pattern, string, expected) in fixtures {
    let actual = wildcard_match(pattern, string);
    dbg!(pattern, string, expected);
    assert_eq!(actual, expected);
  }
}
