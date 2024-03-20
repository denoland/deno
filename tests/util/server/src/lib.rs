// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Usage: provide a port as argument to run hyper_hello benchmark server
// otherwise this starts multiple servers on many ports for test endpoints.
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use once_cell::sync::Lazy;
use pretty_assertions::assert_eq;
use pty::Pty;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::result::Result;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tokio::net::TcpStream;
use url::Url;

pub mod assertions;
mod builders;
pub mod factory;
mod fs;
mod https;
pub mod lsp;
mod macros;
mod npm;
pub mod pty;
pub mod servers;
pub mod spawn;

pub use builders::DenoChild;
pub use builders::TestCommandBuilder;
pub use builders::TestCommandOutput;
pub use builders::TestContext;
pub use builders::TestContextBuilder;
pub use fs::PathRef;
pub use fs::TempDir;

pub const PERMISSION_VARIANTS: [&str; 5] =
  ["read", "write", "env", "net", "run"];
pub const PERMISSION_DENIED_PATTERN: &str = "PermissionDenied";

static GUARD: Lazy<Mutex<HttpServerCount>> =
  Lazy::new(|| Mutex::new(HttpServerCount::default()));

pub fn env_vars_for_npm_tests() -> Vec<(String, String)> {
  vec![
    ("NPM_CONFIG_REGISTRY".to_string(), npm_registry_url()),
    ("NO_COLOR".to_string(), "1".to_string()),
  ]
}

pub fn env_vars_for_jsr_tests_with_git_check() -> Vec<(String, String)> {
  vec![
    ("JSR_URL".to_string(), jsr_registry_url()),
    ("DISABLE_JSR_PROVENANCE".to_string(), "true".to_string()),
    ("NO_COLOR".to_string(), "1".to_string()),
  ]
}

pub fn env_vars_for_jsr_tests() -> Vec<(String, String)> {
  let mut vars = env_vars_for_jsr_tests_with_git_check();

  vars.push((
    "DENO_TESTING_DISABLE_GIT_CHECK".to_string(),
    "1".to_string(),
  ));

  vars
}

pub fn env_vars_for_jsr_provenance_tests() -> Vec<(String, String)> {
  let mut envs = env_vars_for_jsr_tests();
  envs.retain(|(key, _)| key != "DISABLE_JSR_PROVENANCE");
  envs.extend(vec![
    ("REKOR_URL".to_string(), rekor_url()),
    ("FULCIO_URL".to_string(), fulcio_url()),
    (
      "DISABLE_JSR_MANIFEST_VERIFICATION_FOR_TESTING".to_string(),
      "true".to_string(),
    ),
  ]);
  // set GHA variable for attestation.
  envs.extend([
    ("CI".to_string(), "true".to_string()),
    ("GITHUB_ACTIONS".to_string(), "true".to_string()),
    ("ACTIONS_ID_TOKEN_REQUEST_URL".to_string(), gha_token_url()),
    (
      "ACTIONS_ID_TOKEN_REQUEST_TOKEN".to_string(),
      "dummy".to_string(),
    ),
    (
      "GITHUB_REPOSITORY".to_string(),
      "littledivy/deno_sdl2".to_string(),
    ),
    (
      "GITHUB_SERVER_URL".to_string(),
      "https://github.com".to_string(),
    ),
    ("GITHUB_REF".to_string(), "refs/tags/sdl2@0.0.1".to_string()),
    ("GITHUB_SHA".to_string(), "lol".to_string()),
    ("GITHUB_RUN_ID".to_string(), "1".to_string()),
    ("GITHUB_RUN_ATTEMPT".to_string(), "1".to_string()),
    (
      "RUNNER_ENVIRONMENT".to_string(),
      "github-hosted".to_string(),
    ),
    (
      "GITHUB_WORKFLOW_REF".to_string(),
      "littledivy/deno_sdl2@refs/tags/sdl2@0.0.1".to_string(),
    ),
  ]);

  envs
}

pub fn env_vars_for_jsr_npm_tests() -> Vec<(String, String)> {
  vec![
    ("NPM_CONFIG_REGISTRY".to_string(), npm_registry_url()),
    ("JSR_URL".to_string(), jsr_registry_url()),
    (
      "DENO_TESTING_DISABLE_GIT_CHECK".to_string(),
      "1".to_string(),
    ),
    ("DISABLE_JSR_PROVENANCE".to_string(), "true".to_string()),
    ("NO_COLOR".to_string(), "1".to_string()),
  ]
}

pub fn root_path() -> PathRef {
  PathRef::new(
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR")))
      .parent()
      .unwrap()
      .parent()
      .unwrap()
      .parent()
      .unwrap(),
  )
}

pub fn prebuilt_path() -> PathRef {
  third_party_path().join("prebuilt")
}

pub fn tests_path() -> PathRef {
  root_path().join("tests")
}

pub fn testdata_path() -> PathRef {
  tests_path().join("testdata")
}

pub fn third_party_path() -> PathRef {
  root_path().join("third_party")
}

pub fn ffi_tests_path() -> PathRef {
  root_path().join("tests").join("ffi")
}

pub fn napi_tests_path() -> PathRef {
  root_path().join("tests").join("napi")
}

pub fn deno_config_path() -> PathRef {
  root_path().join("tests").join("config").join("deno.json")
}

/// Test server registry url.
pub fn npm_registry_url() -> String {
  "http://localhost:4545/npm/registry/".to_string()
}

pub fn npm_registry_unset_url() -> String {
  "http://NPM_CONFIG_REGISTRY.is.unset".to_string()
}

pub fn jsr_registry_url() -> String {
  "http://127.0.0.1:4250/".to_string()
}

pub fn rekor_url() -> String {
  "http://127.0.0.1:4251".to_string()
}

pub fn fulcio_url() -> String {
  "http://127.0.0.1:4251".to_string()
}

pub fn gha_token_url() -> String {
  "http://127.0.0.1:4251/gha_oidc?test=true".to_string()
}

pub fn jsr_registry_unset_url() -> String {
  "http://JSR_URL.is.unset".to_string()
}

pub fn std_path() -> PathRef {
  root_path().join("tests").join("util").join("std")
}

pub fn std_file_url() -> String {
  Url::from_directory_path(std_path()).unwrap().to_string()
}

pub fn target_dir() -> PathRef {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  PathRef::new(target_dir)
}

pub fn deno_exe_path() -> PathRef {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno").to_path_buf();
  if cfg!(windows) {
    p.set_extension("exe");
  }
  PathRef::new(p)
}

pub fn denort_exe_path() -> PathRef {
  let mut p = target_dir().join("denort").to_path_buf();
  if cfg!(windows) {
    p.set_extension("exe");
  }
  PathRef::new(p)
}

pub fn prebuilt_tool_path(tool: &str) -> PathRef {
  let mut exe = tool.to_string();
  exe.push_str(if cfg!(windows) { ".exe" } else { "" });
  prebuilt_path().join(platform_dir_name()).join(exe)
}

pub fn platform_dir_name() -> &'static str {
  if cfg!(target_os = "linux") {
    "linux64"
  } else if cfg!(target_os = "macos") {
    "mac"
  } else if cfg!(target_os = "windows") {
    "win"
  } else {
    unreachable!()
  }
}

pub fn test_server_path() -> PathBuf {
  let mut p = target_dir().join("test_server").to_path_buf();
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

fn ensure_test_server_built() {
  // if the test server doesn't exist then remind the developer to build first
  if !test_server_path().exists() {
    panic!(
      "Test server not found. Please cargo build before running the tests."
    );
  }
}

/// Returns a [`Stream`] of [`TcpStream`]s accepted from the given port.
async fn get_tcp_listener_stream(
  name: &'static str,
  port: u16,
) -> impl Stream<Item = Result<TcpStream, std::io::Error>> + Unpin + Send {
  let host_and_port = &format!("localhost:{port}");

  // Listen on ALL addresses that localhost can resolves to.
  let accept = |listener: tokio::net::TcpListener| {
    async {
      let result = listener.accept().await;
      Some((result.map(|r| r.0), listener))
    }
    .boxed()
  };

  let mut addresses = vec![];
  let listeners = tokio::net::lookup_host(host_and_port)
    .await
    .expect(host_and_port)
    .inspect(|address| addresses.push(*address))
    .map(tokio::net::TcpListener::bind)
    .collect::<futures::stream::FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(|s| s.unwrap())
    .map(|listener| futures::stream::unfold(listener, accept))
    .collect::<Vec<_>>();

  // Eye catcher for HttpServerCount
  println!("ready: {name} on {:?}", addresses);

  futures::stream::select_all(listeners)
}

#[derive(Default)]
struct HttpServerCount {
  count: usize,
  test_server: Option<Child>,
}

impl HttpServerCount {
  fn inc(&mut self) {
    self.count += 1;
    if self.test_server.is_none() {
      assert_eq!(self.count, 1);

      println!("test_server starting...");
      let mut test_server = Command::new(test_server_path())
        .current_dir(testdata_path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute test_server");
      let stdout = test_server.stdout.as_mut().unwrap();
      use std::io::BufRead;
      use std::io::BufReader;
      let lines = BufReader::new(stdout).lines();

      // Wait for all the servers to report being ready.
      let mut ready_count = 0;
      for maybe_line in lines {
        if let Ok(line) = maybe_line {
          if line.starts_with("ready:") {
            ready_count += 1;
          }
          if ready_count == 12 {
            break;
          }
        } else {
          panic!("{}", maybe_line.unwrap_err());
        }
      }
      self.test_server = Some(test_server);
    }
  }

  fn dec(&mut self) {
    assert!(self.count > 0);
    self.count -= 1;
    if self.count == 0 {
      let mut test_server = self.test_server.take().unwrap();
      match test_server.try_wait() {
        Ok(None) => {
          test_server.kill().expect("failed to kill test_server");
          let _ = test_server.wait();
        }
        Ok(Some(status)) => {
          panic!("test_server exited unexpectedly {status}")
        }
        Err(e) => panic!("test_server error: {e}"),
      }
    }
  }
}

impl Drop for HttpServerCount {
  fn drop(&mut self) {
    assert_eq!(self.count, 0);
    assert!(self.test_server.is_none());
  }
}

fn lock_http_server<'a>() -> MutexGuard<'a, HttpServerCount> {
  let r = GUARD.lock();
  if let Err(poison_err) = r {
    // If panics happened, ignore it. This is for tests.
    poison_err.into_inner()
  } else {
    r.unwrap()
  }
}

pub struct HttpServerGuard {}

impl Drop for HttpServerGuard {
  fn drop(&mut self) {
    let mut g = lock_http_server();
    g.dec();
  }
}

/// Adds a reference to a shared target/debug/test_server subprocess. When the
/// last instance of the HttpServerGuard is dropped, the subprocess will be
/// killed.
pub fn http_server() -> HttpServerGuard {
  ensure_test_server_built();
  let mut g = lock_http_server();
  g.inc();
  HttpServerGuard {}
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<str> {
  console_static_text::ansi::strip_ansi_codes(s)
}

pub fn run(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder.args(&cmd[1..]).stdin(Stdio::piped());

  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let status = prog.wait().expect("failed to wait on child");
  if expect_success != status.success() {
    panic!("Unexpected exit code: {:?}", status.code());
  }
}

pub fn run_collect(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) -> (String, String) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder
    .args(&cmd[1..])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = prog.wait_with_output().expect("failed to wait on child");
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{stdout}>>>");
    eprintln!("stderr: <<<{stderr}>>>");
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn run_and_collect_output(
  expect_success: bool,
  args: &str,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  run_and_collect_output_with_args(
    expect_success,
    args.split_whitespace().collect(),
    input,
    envs,
    need_http_server,
  )
}

pub fn run_and_collect_output_with_args(
  expect_success: bool,
  args: Vec<&str>,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  let mut deno_process_builder = deno_cmd()
    .args_vec(args)
    .current_dir(testdata_path())
    .stdin(Stdio::piped())
    .piped_output();
  if let Some(envs) = envs {
    deno_process_builder = deno_process_builder.envs(envs);
  }
  let _http_guard = if need_http_server {
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
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{stdout}>>>");
    eprintln!("stderr: <<<{stderr}>>>");
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn new_deno_dir() -> TempDir {
  TempDir::new()
}

pub fn deno_cmd() -> TestCommandBuilder {
  let deno_dir = new_deno_dir();
  deno_cmd_with_deno_dir(&deno_dir)
}

pub fn deno_cmd_with_deno_dir(deno_dir: &TempDir) -> TestCommandBuilder {
  TestCommandBuilder::new(deno_dir.clone())
    .env("DENO_DIR", deno_dir.path())
    .env("NPM_CONFIG_REGISTRY", npm_registry_unset_url())
    .env("JSR_URL", jsr_registry_unset_url())
}

pub fn run_powershell_script_file(
  script_file_path: &str,
  args: Vec<&str>,
) -> std::result::Result<(), i64> {
  let deno_dir = new_deno_dir();
  let mut command = Command::new("powershell.exe");

  command
    .env("DENO_DIR", deno_dir.path())
    .current_dir(testdata_path())
    .arg("-file")
    .arg(script_file_path);

  for arg in args {
    command.arg(arg);
  }

  let output = command.output().expect("failed to spawn script");
  let stdout = String::from_utf8(output.stdout).unwrap();
  let stderr = String::from_utf8(output.stderr).unwrap();
  println!("{stdout}");
  if !output.status.success() {
    panic!(
      "{script_file_path} executed with failing error code\n{stdout}{stderr}"
    );
  }

  Ok(())
}

#[derive(Debug, Default)]
pub struct CheckOutputIntegrationTest<'a> {
  pub args: &'a str,
  pub args_vec: Vec<&'a str>,
  pub output: &'a str,
  pub input: Option<&'a str>,
  pub output_str: Option<&'a str>,
  pub exit_code: i32,
  pub http_server: bool,
  pub envs: Vec<(String, String)>,
  pub env_clear: bool,
  pub temp_cwd: bool,
  /// Copies the files at the specified directory in the "testdata" directory
  /// to the temp folder and runs the test from there. This is useful when
  /// the test creates files in the testdata directory (ex. a node_modules folder)
  pub copy_temp_dir: Option<&'a str>,
  /// Relative to "testdata" directory
  pub cwd: Option<&'a str>,
}

impl<'a> CheckOutputIntegrationTest<'a> {
  pub fn output(&self) -> TestCommandOutput {
    let mut context_builder = TestContextBuilder::default();
    if self.temp_cwd {
      context_builder = context_builder.use_temp_cwd();
    }
    if let Some(dir) = &self.copy_temp_dir {
      context_builder = context_builder.use_copy_temp_dir(dir);
    }
    if self.http_server {
      context_builder = context_builder.use_http_server();
    }

    let context = context_builder.build();

    let mut command_builder = context.new_command();

    if !self.args.is_empty() {
      command_builder = command_builder.args(self.args);
    }
    if !self.args_vec.is_empty() {
      command_builder = command_builder.args_vec(self.args_vec.clone());
    }
    if let Some(input) = &self.input {
      command_builder = command_builder.stdin_text(input);
    }
    for (key, value) in &self.envs {
      command_builder = command_builder.env(key, value);
    }
    if self.env_clear {
      command_builder = command_builder.env_clear();
    }
    if let Some(cwd) = &self.cwd {
      command_builder = command_builder.current_dir(cwd);
    }

    command_builder.run()
  }
}

pub fn wildcard_match(pattern: &str, text: &str) -> bool {
  match wildcard_match_detailed(pattern, text) {
    WildcardMatchResult::Success => true,
    WildcardMatchResult::Fail(debug_output) => {
      eprintln!("{}", debug_output);
      false
    }
  }
}

pub enum WildcardMatchResult {
  Success,
  Fail(String),
}

pub fn wildcard_match_detailed(
  pattern: &str,
  text: &str,
) -> WildcardMatchResult {
  fn annotate_whitespace(text: &str) -> String {
    text.replace('\t', "\u{2192}").replace(' ', "\u{00B7}")
  }

  // Normalize line endings
  let original_text = text.replace("\r\n", "\n");
  let mut current_text = original_text.as_str();
  // normalize line endings and strip comments
  let pattern = pattern
    .split('\n')
    .map(|line| line.trim_end_matches('\r'))
    .filter(|l| {
      let is_comment = l.starts_with("[#") && l.ends_with(']');
      !is_comment
    })
    .collect::<Vec<_>>()
    .join("\n");
  let mut output_lines = Vec::new();

  let parts = parse_wildcard_pattern_text(&pattern).unwrap();

  let mut was_last_wildcard = false;
  let mut was_last_wildline = false;
  for (i, part) in parts.iter().enumerate() {
    match part {
      WildcardPatternPart::Wildcard => {
        output_lines.push("<WILDCARD />".to_string());
      }
      WildcardPatternPart::Wildline => {
        output_lines.push("<WILDLINE />".to_string());
      }
      WildcardPatternPart::Wildnum(times) => {
        if current_text.len() < *times {
          output_lines
            .push(format!("==== HAD MISSING WILDCHARS({}) ====", times));
          output_lines.push(colors::red(annotate_whitespace(current_text)));
          return WildcardMatchResult::Fail(output_lines.join("\n"));
        }
        output_lines.push(format!("<WILDCHARS({}) />", times));
        current_text = &current_text[*times..];
      }
      WildcardPatternPart::Text(search_text) => {
        let is_last = i + 1 == parts.len();
        let search_index = if is_last && was_last_wildcard {
          // search from the end of the file
          current_text.rfind(search_text)
        } else if was_last_wildline {
          if is_last {
            find_last_text_on_line(search_text, current_text)
          } else {
            find_first_text_on_line(search_text, current_text)
          }
        } else {
          current_text.find(search_text)
        };
        match search_index {
          Some(found_index)
            if was_last_wildcard || was_last_wildline || found_index == 0 =>
          {
            output_lines.push(format!(
              "<FOUND>{}</FOUND>",
              colors::gray(annotate_whitespace(search_text))
            ));
            current_text = &current_text[found_index + search_text.len()..];
          }
          Some(index) => {
            output_lines.push(
              "==== FOUND SEARCH TEXT IN WRONG POSITION ====".to_string(),
            );
            output_lines.push(colors::gray(annotate_whitespace(search_text)));
            output_lines
              .push("==== HAD UNKNOWN PRECEDING TEXT ====".to_string());
            output_lines
              .push(colors::red(annotate_whitespace(&current_text[..index])));
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          }
          None => {
            let was_wildcard_or_line = was_last_wildcard || was_last_wildline;
            let mut max_found_index = 0;
            for (index, _) in search_text.char_indices() {
              let sub_string = &search_text[..index];
              if let Some(found_index) = current_text.find(sub_string) {
                if was_wildcard_or_line || found_index == 0 {
                  max_found_index = index;
                } else {
                  break;
                }
              } else {
                break;
              }
            }
            if !was_wildcard_or_line && max_found_index > 0 {
              output_lines.push(format!(
                "<FOUND>{}</FOUND>",
                colors::gray(annotate_whitespace(
                  &search_text[..max_found_index]
                ))
              ));
            }
            output_lines
              .push("==== COULD NOT FIND SEARCH TEXT ====".to_string());
            output_lines.push(colors::green(annotate_whitespace(
              if was_wildcard_or_line {
                search_text
              } else {
                &search_text[max_found_index..]
              },
            )));
            if was_wildcard_or_line && max_found_index > 0 {
              output_lines.push(format!(
                "==== MAX FOUND ====\n{}",
                colors::red(annotate_whitespace(
                  &search_text[..max_found_index]
                ))
              ));
            }
            let actual_next_text = &current_text[max_found_index..];
            let max_next_text_len = 40;
            let next_text_len =
              std::cmp::min(max_next_text_len, actual_next_text.len());
            output_lines.push(format!(
              "==== NEXT ACTUAL TEXT ====\n{}{}",
              colors::red(annotate_whitespace(
                &actual_next_text[..next_text_len]
              )),
              if actual_next_text.len() > max_next_text_len {
                "[TRUNCATED]"
              } else {
                ""
              },
            ));
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          }
        }
      }
      WildcardPatternPart::UnorderedLines(expected_lines) => {
        assert!(!was_last_wildcard, "unsupported");
        assert!(!was_last_wildline, "unsupported");
        let mut actual_lines = Vec::with_capacity(expected_lines.len());
        for _ in 0..expected_lines.len() {
          match current_text.find('\n') {
            Some(end_line_index) => {
              actual_lines.push(&current_text[..end_line_index]);
              current_text = &current_text[end_line_index + 1..];
            }
            None => {
              break;
            }
          }
        }
        actual_lines.sort_unstable();
        let mut expected_lines = expected_lines.clone();
        expected_lines.sort_unstable();

        if actual_lines.len() != expected_lines.len() {
          output_lines
            .push("==== HAD WRONG NUMBER OF UNORDERED LINES ====".to_string());
          output_lines.push("# ACTUAL".to_string());
          output_lines.extend(
            actual_lines
              .iter()
              .map(|l| colors::green(annotate_whitespace(l))),
          );
          output_lines.push("# EXPECTED".to_string());
          output_lines.extend(
            expected_lines
              .iter()
              .map(|l| colors::green(annotate_whitespace(l))),
          );
          return WildcardMatchResult::Fail(output_lines.join("\n"));
        }
        for (actual, expected) in actual_lines.iter().zip(expected_lines.iter())
        {
          if actual != expected {
            output_lines
              .push("==== UNORDERED LINE DID NOT MATCH ====".to_string());
            output_lines.push(format!(
              "  ACTUAL: {}",
              colors::red(annotate_whitespace(actual))
            ));
            output_lines.push(format!(
              "EXPECTED: {}",
              colors::green(annotate_whitespace(expected))
            ));
            return WildcardMatchResult::Fail(output_lines.join("\n"));
          } else {
            output_lines.push(format!(
              "<FOUND>{}</FOUND>",
              colors::gray(annotate_whitespace(expected))
            ));
          }
        }
      }
    }
    was_last_wildcard = matches!(part, WildcardPatternPart::Wildcard);
    was_last_wildline = matches!(part, WildcardPatternPart::Wildline);
  }

  if was_last_wildcard || was_last_wildline || current_text.is_empty() {
    WildcardMatchResult::Success
  } else {
    output_lines.push("==== HAD TEXT AT END OF FILE ====".to_string());
    output_lines.push(colors::red(annotate_whitespace(current_text)));
    WildcardMatchResult::Fail(output_lines.join("\n"))
  }
}

#[derive(Debug)]
enum WildcardPatternPart<'a> {
  Wildcard,
  Wildline,
  Wildnum(usize),
  Text(&'a str),
  UnorderedLines(Vec<&'a str>),
}

fn parse_wildcard_pattern_text(
  text: &str,
) -> Result<Vec<WildcardPatternPart>, monch::ParseErrorFailureError> {
  use monch::*;

  fn parse_unordered_lines(input: &str) -> ParseResult<Vec<&str>> {
    const END_TEXT: &str = "\n[UNORDERED_END]\n";
    let (input, _) = tag("[UNORDERED_START]\n")(input)?;
    match input.find(END_TEXT) {
      Some(end_index) => ParseResult::Ok((
        &input[end_index + END_TEXT.len()..],
        input[..end_index].lines().collect::<Vec<_>>(),
      )),
      None => ParseError::fail(input, "Could not find [UNORDERED_END]"),
    }
  }

  enum InnerPart<'a> {
    Wildcard,
    Wildline,
    Wildnum(usize),
    UnorderedLines(Vec<&'a str>),
    Char,
  }

  struct Parser<'a> {
    current_input: &'a str,
    last_text_input: &'a str,
    parts: Vec<WildcardPatternPart<'a>>,
  }

  impl<'a> Parser<'a> {
    fn parse(mut self) -> ParseResult<'a, Vec<WildcardPatternPart<'a>>> {
      fn parse_num(input: &str) -> ParseResult<usize> {
        let num_char_count =
          input.chars().take_while(|c| c.is_ascii_digit()).count();
        if num_char_count == 0 {
          return ParseError::backtrace();
        }
        let (char_text, input) = input.split_at(num_char_count);
        let value = str::parse::<usize>(char_text).unwrap();
        Ok((input, value))
      }

      fn parse_wild_num(input: &str) -> ParseResult<usize> {
        let (input, _) = tag("[WILDCHARS(")(input)?;
        let (input, times) = parse_num(input)?;
        let (input, _) = tag(")]")(input)?;
        ParseResult::Ok((input, times))
      }

      while !self.current_input.is_empty() {
        let (next_input, inner_part) = or5(
          map(tag("[WILDCARD]"), |_| InnerPart::Wildcard),
          map(tag("[WILDLINE]"), |_| InnerPart::Wildline),
          map(parse_wild_num, InnerPart::Wildnum),
          map(parse_unordered_lines, |lines| {
            InnerPart::UnorderedLines(lines)
          }),
          map(next_char, |_| InnerPart::Char),
        )(self.current_input)?;
        match inner_part {
          InnerPart::Wildcard => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildcard);
          }
          InnerPart::Wildline => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildline);
          }
          InnerPart::Wildnum(times) => {
            self.queue_previous_text(next_input);
            self.parts.push(WildcardPatternPart::Wildnum(times));
          }
          InnerPart::UnorderedLines(expected_lines) => {
            self.queue_previous_text(next_input);
            self
              .parts
              .push(WildcardPatternPart::UnorderedLines(expected_lines));
          }
          InnerPart::Char => {
            // ignore
          }
        }
        self.current_input = next_input;
      }

      self.queue_previous_text("");

      ParseResult::Ok(("", self.parts))
    }

    fn queue_previous_text(&mut self, next_input: &'a str) {
      let previous_text = &self.last_text_input
        [..self.last_text_input.len() - self.current_input.len()];
      if !previous_text.is_empty() {
        self.parts.push(WildcardPatternPart::Text(previous_text));
      }
      self.last_text_input = next_input;
    }
  }

  with_failure_handling(|input| {
    Parser {
      current_input: input,
      last_text_input: input,
      parts: Vec::new(),
    }
    .parse()
  })(text)
}

fn find_first_text_on_line(
  search_text: &str,
  current_text: &str,
) -> Option<usize> {
  let end_search_pos = current_text.find('\n').unwrap_or(current_text.len());
  let found_pos = current_text.find(search_text)?;
  if found_pos <= end_search_pos {
    Some(found_pos)
  } else {
    None
  }
}

fn find_last_text_on_line(
  search_text: &str,
  current_text: &str,
) -> Option<usize> {
  let end_search_pos = current_text.find('\n').unwrap_or(current_text.len());
  let mut best_match = None;
  let mut search_pos = 0;
  while let Some(new_pos) = current_text[search_pos..].find(search_text) {
    search_pos += new_pos;
    if search_pos <= end_search_pos {
      best_match = Some(search_pos);
    } else {
      break;
    }
    search_pos += 1;
  }
  best_match
}

pub fn with_pty(deno_args: &[&str], action: impl FnMut(Pty)) {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  context.new_command().args_vec(deno_args).with_pty(action);
}

pub struct WrkOutput {
  pub latency: f64,
  pub requests: u64,
}

pub fn parse_wrk_output(output: &str) -> WrkOutput {
  static REQUESTS_RX: Lazy<Regex> =
    lazy_regex::lazy_regex!(r"Requests/sec:\s+(\d+)");
  static LATENCY_RX: Lazy<Regex> =
    lazy_regex::lazy_regex!(r"\s+99%(?:\s+(\d+.\d+)([a-z]+))");

  let mut requests = None;
  let mut latency = None;

  for line in output.lines() {
    if requests.is_none() {
      if let Some(cap) = REQUESTS_RX.captures(line) {
        requests =
          Some(str::parse::<u64>(cap.get(1).unwrap().as_str()).unwrap());
      }
    }
    if latency.is_none() {
      if let Some(cap) = LATENCY_RX.captures(line) {
        let time = cap.get(1).unwrap();
        let unit = cap.get(2).unwrap();

        latency = Some(
          str::parse::<f64>(time.as_str()).unwrap()
            * match unit.as_str() {
              "ms" => 1.0,
              "us" => 0.001,
              "s" => 1000.0,
              _ => unreachable!(),
            },
        );
      }
    }
  }

  WrkOutput {
    requests: requests.unwrap(),
    latency: latency.unwrap(),
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct StraceOutput {
  pub percent_time: f64,
  pub seconds: f64,
  pub usecs_per_call: Option<u64>,
  pub calls: u64,
  pub errors: u64,
}

pub fn parse_strace_output(output: &str) -> HashMap<String, StraceOutput> {
  let mut summary = HashMap::new();

  // Filter out non-relevant lines. See the error log at
  // https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
  // This is checked in testdata/strace_summary2.out
  let mut lines = output.lines().filter(|line| {
    !line.is_empty()
      && !line.contains("detached ...")
      && !line.contains("unfinished ...")
      && !line.contains("????")
  });
  let count = lines.clone().count();

  if count < 4 {
    return summary;
  }

  let total_line = lines.next_back().unwrap();
  lines.next_back(); // Drop separator
  let data_lines = lines.skip(2);

  for line in data_lines {
    let syscall_fields = line.split_whitespace().collect::<Vec<_>>();
    let len = syscall_fields.len();
    let syscall_name = syscall_fields.last().unwrap();
    if (5..=6).contains(&len) {
      summary.insert(
        syscall_name.to_string(),
        StraceOutput {
          percent_time: str::parse::<f64>(syscall_fields[0]).unwrap(),
          seconds: str::parse::<f64>(syscall_fields[1]).unwrap(),
          usecs_per_call: Some(str::parse::<u64>(syscall_fields[2]).unwrap()),
          calls: str::parse::<u64>(syscall_fields[3]).unwrap(),
          errors: if syscall_fields.len() < 6 {
            0
          } else {
            str::parse::<u64>(syscall_fields[4]).unwrap()
          },
        },
      );
    }
  }

  let total_fields = total_line.split_whitespace().collect::<Vec<_>>();

  let mut usecs_call_offset = 0;
  summary.insert(
    "total".to_string(),
    StraceOutput {
      percent_time: str::parse::<f64>(total_fields[0]).unwrap(),
      seconds: str::parse::<f64>(total_fields[1]).unwrap(),
      usecs_per_call: if total_fields.len() > 5 {
        usecs_call_offset = 1;
        Some(str::parse::<u64>(total_fields[2]).unwrap())
      } else {
        None
      },
      calls: str::parse::<u64>(total_fields[2 + usecs_call_offset]).unwrap(),
      errors: str::parse::<u64>(total_fields[3 + usecs_call_offset]).unwrap(),
    },
  );

  summary
}

pub fn parse_max_mem(output: &str) -> Option<u64> {
  // Takes the output from "time -v" as input and extracts the 'maximum
  // resident set size' and returns it in bytes.
  for line in output.lines() {
    if line
      .to_lowercase()
      .contains("maximum resident set size (kbytes)")
    {
      let value = line.split(": ").nth(1).unwrap();
      return Some(str::parse::<u64>(value).unwrap() * 1024);
    }
  }

  None
}

pub(crate) mod colors {
  use std::io::Write;

  use termcolor::Ansi;
  use termcolor::Color;
  use termcolor::ColorSpec;
  use termcolor::WriteColor;

  pub fn bold<S: AsRef<str>>(s: S) -> String {
    let mut style_spec = ColorSpec::new();
    style_spec.set_bold(true);
    style(s, style_spec)
  }

  pub fn red<S: AsRef<str>>(s: S) -> String {
    fg_color(s, Color::Red)
  }

  pub fn bold_red<S: AsRef<str>>(s: S) -> String {
    bold_fg_color(s, Color::Red)
  }

  pub fn green<S: AsRef<str>>(s: S) -> String {
    fg_color(s, Color::Green)
  }

  pub fn bold_green<S: AsRef<str>>(s: S) -> String {
    bold_fg_color(s, Color::Green)
  }

  pub fn bold_blue<S: AsRef<str>>(s: S) -> String {
    bold_fg_color(s, Color::Blue)
  }

  pub fn gray<S: AsRef<str>>(s: S) -> String {
    fg_color(s, Color::Ansi256(245))
  }

  fn bold_fg_color<S: AsRef<str>>(s: S, color: Color) -> String {
    let mut style_spec = ColorSpec::new();
    style_spec.set_bold(true);
    style_spec.set_fg(Some(color));
    style(s, style_spec)
  }

  fn fg_color<S: AsRef<str>>(s: S, color: Color) -> String {
    let mut style_spec = ColorSpec::new();
    style_spec.set_fg(Some(color));
    style(s, style_spec)
  }

  fn style<S: AsRef<str>>(s: S, colorspec: ColorSpec) -> String {
    let mut v = Vec::new();
    let mut ansi_writer = Ansi::new(&mut v);
    ansi_writer.set_color(&colorspec).unwrap();
    ansi_writer.write_all(s.as_ref().as_bytes()).unwrap();
    ansi_writer.reset().unwrap();
    String::from_utf8_lossy(&v).into_owned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn parse_wrk_output_1() {
    const TEXT: &str = include_str!("./testdata/wrk1.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 1837);
    assert!((wrk.latency - 6.25).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_2() {
    const TEXT: &str = include_str!("./testdata/wrk2.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 53435);
    assert!((wrk.latency - 6.22).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_3() {
    const TEXT: &str = include_str!("./testdata/wrk3.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 96037);
    assert!((wrk.latency - 6.36).abs() < f64::EPSILON);
  }

  #[test]
  fn strace_parse_1() {
    const TEXT: &str = include_str!("./testdata/strace_summary.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let munmap = strace.get("munmap").unwrap();
    assert_eq!(munmap.calls, 60);
    assert_eq!(munmap.errors, 0);

    // line with errors
    assert_eq!(strace.get("mkdir").unwrap().errors, 2);

    // last syscall line
    let prlimit = strace.get("prlimit64").unwrap();
    assert_eq!(prlimit.calls, 2);
    assert!((prlimit.percent_time - 0.0).abs() < f64::EPSILON);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 704);
    assert_eq!(strace.get("total").unwrap().errors, 5);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, None);
  }

  #[test]
  fn strace_parse_2() {
    const TEXT: &str = include_str!("./testdata/strace_summary2.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("futex").unwrap();
    assert_eq!(futex.calls, 449);
    assert_eq!(futex.errors, 94);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 821);
    assert_eq!(strace.get("total").unwrap().errors, 107);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, None);
  }

  #[test]
  fn strace_parse_3() {
    const TEXT: &str = include_str!("./testdata/strace_summary3.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("mprotect").unwrap();
    assert_eq!(futex.calls, 90);
    assert_eq!(futex.errors, 0);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 543);
    assert_eq!(strace.get("total").unwrap().errors, 36);
    assert_eq!(strace.get("total").unwrap().usecs_per_call, Some(6));
  }

  #[test]
  fn parse_parse_wildcard_match_text() {
    let result =
      parse_wildcard_pattern_text("[UNORDERED_START]\ntesting\ntesting")
        .err()
        .unwrap();
    assert_contains!(result.to_string(), "Could not find [UNORDERED_END]");
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
  fn test_wildcard_match2() {
    // foo, bar, baz, qux, quux, quuz, corge, grault, garply, waldo, fred, plugh, xyzzy

    assert!(wildcard_match("foo[WILDCARD]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCARD]baz", "foobazbar"));

    let multiline_pattern = "[WILDCARD]
foo:
[WILDCARD]baz[WILDCARD]";

    fn multi_line_builder(input: &str, leading_text: Option<&str>) -> String {
      // If there is leading text add a newline so it's on it's own line
      let head = match leading_text {
        Some(v) => format!("{v}\n"),
        None => "".to_string(),
      };
      format!(
        "{head}foo:
quuz {input} corge
grault"
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
    assert!(wildcard_match(
      multiline_pattern,
      &multi_line_builder("baz", Some("QUX=quux")),
    ));

    // Should fail when leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("baz", None),
    ));

    // Incorrect input & leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("garply", Some("QUX=quux")),
    ));

    // Incorrect input & no leading line
    assert!(!wildcard_match(
      multiline_pattern,
      &multi_line_builder("garply", None),
    ));

    // wildline
    assert!(wildcard_match("foo[WILDLINE]baz", "foobarbaz"));
    assert!(wildcard_match("foo[WILDLINE]bar", "foobarbar"));
    assert!(!wildcard_match("foo[WILDLINE]baz", "fooba\nrbaz"));
    assert!(wildcard_match("foo[WILDLINE]", "foobar"));

    // wildnum
    assert!(wildcard_match("foo[WILDCHARS(3)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(4)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(2)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(1)]baz", "foobarbaz"));
    assert!(!wildcard_match("foo[WILDCHARS(20)]baz", "foobarbaz"));
  }

  #[test]
  fn test_wildcard_match_unordered_lines() {
    // matching
    assert!(wildcard_match(
      concat!("[UNORDERED_START]\n", "B\n", "A\n", "[UNORDERED_END]\n"),
      concat!("A\n", "B\n",)
    ));
    // different line
    assert!(!wildcard_match(
      concat!("[UNORDERED_START]\n", "Ba\n", "A\n", "[UNORDERED_END]\n"),
      concat!("A\n", "B\n",)
    ));
    // different number of lines
    assert!(!wildcard_match(
      concat!(
        "[UNORDERED_START]\n",
        "B\n",
        "A\n",
        "C\n",
        "[UNORDERED_END]\n"
      ),
      concat!("A\n", "B\n",)
    ));
  }

  #[test]
  fn max_mem_parse() {
    const TEXT: &str = include_str!("./testdata/time.out");
    let size = parse_max_mem(TEXT);

    assert_eq!(size, Some(120380 * 1024));
  }

  #[test]
  fn test_find_first_text_on_line() {
    let text = "foo\nbar\nbaz";
    assert_eq!(find_first_text_on_line("foo", text), Some(0));
    assert_eq!(find_first_text_on_line("oo", text), Some(1));
    assert_eq!(find_first_text_on_line("o", text), Some(1));
    assert_eq!(find_first_text_on_line("o\nbar", text), Some(2));
    assert_eq!(find_first_text_on_line("f", text), Some(0));
    assert_eq!(find_first_text_on_line("bar", text), None);
  }

  #[test]
  fn test_find_last_text_on_line() {
    let text = "foo\nbar\nbaz";
    assert_eq!(find_last_text_on_line("foo", text), Some(0));
    assert_eq!(find_last_text_on_line("oo", text), Some(1));
    assert_eq!(find_last_text_on_line("o", text), Some(2));
    assert_eq!(find_last_text_on_line("o\nbar", text), Some(2));
    assert_eq!(find_last_text_on_line("f", text), Some(0));
    assert_eq!(find_last_text_on_line("bar", text), None);
  }
}
