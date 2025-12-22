// Copyright 2018-2025 the Deno authors. MIT license.

use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::result::Result;

use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use parking_lot::MutexGuard;
use pretty_assertions::assert_eq;
use pty::Pty;
use tokio::net::TcpStream;
use url::Url;

pub mod assertions;
mod builders;
mod fs;
mod https;
pub mod lsp;
mod macros;
mod npm;
mod parsers;
pub mod print;
pub mod pty;
mod semaphore;
pub mod servers;
pub mod spawn;
pub mod test_runner;
mod wildcard;

pub use builders::DenoChild;
pub use builders::TestCommandBuilder;
pub use builders::TestCommandOutput;
pub use builders::TestContext;
pub use builders::TestContextBuilder;
pub use fs::PathRef;
pub use fs::TempDir;
pub use fs::url_to_notebook_cell_uri;
pub use fs::url_to_uri;
pub use inventory::submit;
pub use parsers::StraceOutput;
pub use parsers::WrkOutput;
pub use parsers::parse_max_mem;
pub use parsers::parse_strace_output;
pub use parsers::parse_wrk_output;
pub use test_macro::test;
pub use wildcard::WildcardMatchResult;
pub use wildcard::wildcard_match_detailed;

pub const PERMISSION_VARIANTS: [&str; 5] =
  ["read", "write", "env", "net", "run"];
pub const PERMISSION_DENIED_PATTERN: &str = "NotCapable";

static GUARD: Lazy<Mutex<HttpServerCount>> = Lazy::new(Default::default);

pub static IS_CI: Lazy<bool> = Lazy::new(|| std::env::var("CI").is_ok());

pub fn env_vars_for_npm_tests() -> Vec<(String, String)> {
  vec![
    ("NPM_CONFIG_REGISTRY".to_string(), npm_registry_url()),
    ("NODEJS_ORG_MIRROR".to_string(), nodejs_org_mirror_url()),
    ("NO_COLOR".to_string(), "1".to_string()),
    ("SOCKET_DEV_URL".to_string(), socket_dev_api_url()),
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
    ("NODEJS_ORG_MIRROR".to_string(), nodejs_org_mirror_url()),
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
  format!("http://localhost:{}/", servers::PUBLIC_NPM_REGISTRY_PORT)
}

pub fn npm_registry_unset_url() -> String {
  "http://NPM_CONFIG_REGISTRY.is.unset".to_string()
}

pub fn nodejs_org_mirror_url() -> String {
  format!(
    "http://127.0.0.1:{}/",
    servers::NODEJS_ORG_MIRROR_SERVER_PORT
  )
}

pub fn nodejs_org_mirror_unset_url() -> String {
  "http://NODEJS_ORG_MIRROR.is.unset".to_string()
}

pub fn jsr_registry_url() -> String {
  format!("http://127.0.0.1:{}/", servers::JSR_REGISTRY_SERVER_PORT)
}

pub fn rekor_url() -> String {
  format!("http://127.0.0.1:{}", servers::PROVENANCE_MOCK_SERVER_PORT)
}

pub fn fulcio_url() -> String {
  format!("http://127.0.0.1:{}", servers::PROVENANCE_MOCK_SERVER_PORT)
}

pub fn gha_token_url() -> String {
  format!(
    "http://127.0.0.1:{}/gha_oidc?test=true",
    servers::PROVENANCE_MOCK_SERVER_PORT
  )
}

pub fn socket_dev_api_url() -> String {
  format!("http://localhost:{}/", servers::SOCKET_DEV_API_PORT)
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

pub const TEST_SERVERS_COUNT: usize = 35;

#[derive(Default)]
struct HttpServerCount {
  count: usize,
  test_server: Option<HttpServerStarter>,
}

impl HttpServerCount {
  fn inc(&mut self) {
    self.count += 1;
    if self.test_server.is_none() {
      self.test_server = Some(Default::default());
    }
  }

  fn dec(&mut self) {
    assert!(self.count > 0);
    self.count -= 1;
    if self.count == 0 {
      self.test_server.take();
    }
  }
}

impl Drop for HttpServerCount {
  fn drop(&mut self) {
    assert_eq!(self.count, 0);
    assert!(self.test_server.is_none());
  }
}

struct HttpServerStarter {
  test_server: Child,
}

impl Default for HttpServerStarter {
  fn default() -> Self {
    println!("test_server starting...");
    let mut test_server = Command::new(test_server_path())
      .current_dir(testdata_path())
      .stdout(Stdio::piped())
      .spawn()
      .inspect_err(|_| {
        ensure_test_server_built();
      })
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
        if ready_count == TEST_SERVERS_COUNT {
          break;
        }
      } else {
        panic!("{}", maybe_line.unwrap_err());
      }
    }
    Self { test_server }
  }
}

impl Drop for HttpServerStarter {
  fn drop(&mut self) {
    match self.test_server.try_wait() {
      Ok(None) => {
        self.test_server.kill().expect("failed to kill test_server");
        let _ = self.test_server.wait();
      }
      Ok(Some(status)) => {
        panic!("test_server exited unexpectedly {status}")
      }
      Err(e) => panic!("test_server error: {e}"),
    }
  }
}

fn lock_http_server<'a>() -> MutexGuard<'a, HttpServerCount> {
  GUARD.lock()
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
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
  let mut g = lock_http_server();
  g.inc();
  HttpServerGuard {}
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<'_, str> {
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
    .env("NODEJS_ORG_MIRROR", nodejs_org_mirror_unset_url())
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

impl CheckOutputIntegrationTest<'_> {
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

pub fn with_pty(deno_args: &[&str], action: impl FnMut(Pty)) {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  context.new_command().args_vec(deno_args).with_pty(action);
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

  pub fn yellow<S: AsRef<str>>(s: S) -> String {
    fg_color(s, Color::Yellow)
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

#[derive(Debug, Clone)]
pub struct TestMacroCase {
  pub name: &'static str,
  pub module_name: &'static str,
  pub file: &'static str,
  /// 1-indexed
  pub line: u32,
  /// 1-indexed
  pub col: u32,
  pub func: fn(),
  pub flaky: bool,
  pub ignore: bool,
  pub timeout: Option<usize>,
}

inventory::collect!(TestMacroCase);

pub fn collect_and_filter_tests(
  main_category: &mut file_test_runner::collection::CollectedTestCategory<
    &'static TestMacroCase,
  >,
) {
  for test in inventory::iter::<TestMacroCase>() {
    main_category.children.push(
      file_test_runner::collection::CollectedCategoryOrTest::Test(
        file_test_runner::collection::CollectedTest {
          name: format!("{}::{}", test.module_name, test.name),
          path: PathBuf::from(test.file),
          // line and col are 1-indexed, but file_test_runner uses
          // 0-indexed numbers, so keep as-is for line to put it on
          // probably the function name and then do col - 1 to make
          // the column 0-indexed
          line_and_column: Some((test.line, test.col - 1)),
          data: test,
        },
      ),
    );
  }

  if let Some(filter) = file_test_runner::collection::parse_cli_arg_filter() {
    main_category.filter_children(&filter);
  }
}
