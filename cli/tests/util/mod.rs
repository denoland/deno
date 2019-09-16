//! Test utilites shared between integration_tests.rs and tty_tests.rs
use deno_cli::colors::strip_ansi_codes;
pub use deno_cli::test_util::*;
use os_pipe::pipe;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

lazy_static! {
  static ref DENO_DIR: TempDir = { TempDir::new().expect("tempdir fail") };
}

#[allow(dead_code)]
pub fn deno_cmd() -> Command {
  let mut c = Command::new(deno_exe_path());
  c.env("DENO_DIR", DENO_DIR.path());
  c
}

pub fn run_python_script(script: &str) {
  let output = Command::new("python")
    .env("DENO_DIR", DENO_DIR.path())
    .current_dir(root_path())
    .arg(script)
    .arg(format!("--executable={}", deno_exe_path().display()))
    .env("DENO_BUILD_PATH", target_dir())
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
  pub exit_code: i32,
  pub check_stderr: bool,
  pub http_server: bool,
}

impl CheckOutputIntegrationTest {
  #[allow(dead_code)]
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
    command.args(args);
    command.current_dir(&tests_dir);
    command.stdin(Stdio::piped());
    command.stderr(Stdio::null());

    if self.check_stderr {
      let writer_clone = writer.try_clone().unwrap();
      command.stderr(writer_clone);
    }

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

    let output_path = tests_dir.join(self.output);
    println!("output path {}", output_path.display());
    let expected =
      std::fs::read_to_string(output_path).expect("cannot read output");

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

fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
  // Normalize line endings
  let s = s.replace("\r\n", "\n");
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

#[test]
fn test_pattern_match() {
  assert!(pattern_match("foo[BAR]baz", "foobarbaz", "[BAR]"));
  assert!(!pattern_match("foo[BAR]baz", "foobazbar", "[BAR]"));
}
