// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::rc::Rc;

use os_pipe::pipe;

use crate::assertions::assert_wildcard_match;
use crate::deno_exe_path;
use crate::env_vars_for_jsr_tests;
use crate::env_vars_for_npm_tests;
use crate::fs::PathRef;
use crate::http_server;
use crate::lsp::LspClientBuilder;
use crate::npm_registry_unset_url;
use crate::pty::Pty;
use crate::strip_ansi_codes;
use crate::testdata_path;
use crate::HttpServerGuard;
use crate::TempDir;

// Gives the developer a nice error message if they have a deno configuration
// file that will be auto-discovered by the tests and cause a lot of failures.
static HAS_DENO_JSON_IN_WORKING_DIR_ERR: once_cell::sync::Lazy<Option<String>> =
  once_cell::sync::Lazy::new(|| {
    let testdata_path = testdata_path();
    let mut current_dir = testdata_path.as_path();
    let deno_json_names = ["deno.json", "deno.jsonc"];
    loop {
      for name in deno_json_names {
        let deno_json_path = current_dir.join(name);
        if deno_json_path.exists() {
          return Some(format!(
            concat!(
              "Found deno configuration file at {}. The test suite relies on ",
              "a deno.json not existing in any ancestor directory. Please ",
              "delete this file so the tests won't auto-discover it.",
            ),
            deno_json_path.display(),
          ));
        }
      }
      if let Some(parent) = current_dir.parent() {
        current_dir = parent;
      } else {
        break;
      }
    }

    None
  });

#[derive(Default)]
pub struct TestContextBuilder {
  use_http_server: bool,
  use_temp_cwd: bool,
  use_symlinked_temp_dir: bool,
  /// Copies the files at the specified directory in the "testdata" directory
  /// to the temp folder and runs the test from there. This is useful when
  /// the test creates files in the testdata directory (ex. a node_modules folder)
  copy_temp_dir: Option<String>,
  temp_dir_path: Option<PathBuf>,
  cwd: Option<String>,
  envs: HashMap<String, String>,
}

impl TestContextBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn for_npm() -> Self {
    Self::new().use_http_server().add_npm_env_vars()
  }

  pub fn for_jsr() -> Self {
    Self::new().use_http_server().add_jsr_env_vars()
  }

  pub fn temp_dir_path(mut self, path: impl AsRef<Path>) -> Self {
    self.temp_dir_path = Some(path.as_ref().to_path_buf());
    self
  }

  pub fn use_http_server(mut self) -> Self {
    self.use_http_server = true;
    self
  }

  pub fn use_temp_cwd(mut self) -> Self {
    self.use_temp_cwd = true;
    self
  }

  /// Causes the temp directory to be symlinked to a target directory
  /// which is useful for debugging issues that only show up on the CI.
  ///
  /// Note: This method is not actually deprecated, it's just the CI
  /// does this by default so there's no need to check in any code that
  /// uses this into the repo. This is just for debugging purposes.
  #[deprecated]
  pub fn use_symlinked_temp_dir(mut self) -> Self {
    self.use_symlinked_temp_dir = true;
    self
  }

  /// Copies the files at the specified directory in the "testdata" directory
  /// to the temp folder and runs the test from there. This is useful when
  /// the test creates files in the testdata directory (ex. a node_modules folder)
  pub fn use_copy_temp_dir(mut self, dir: impl AsRef<str>) -> Self {
    self.copy_temp_dir = Some(dir.as_ref().to_string());
    self
  }

  pub fn cwd(mut self, cwd: impl AsRef<str>) -> Self {
    self.cwd = Some(cwd.as_ref().to_string());
    self
  }

  pub fn env(mut self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
    self
      .envs
      .insert(key.as_ref().to_string(), value.as_ref().to_string());
    self
  }

  pub fn add_npm_env_vars(mut self) -> Self {
    for (key, value) in env_vars_for_npm_tests() {
      self = self.env(key, value);
    }
    self
  }

  pub fn add_jsr_env_vars(mut self) -> Self {
    for (key, value) in env_vars_for_jsr_tests() {
      self = self.env(key, value);
    }
    self
  }

  pub fn build(&self) -> TestContext {
    if let Some(err) = &*HAS_DENO_JSON_IN_WORKING_DIR_ERR {
      panic!("{}", err);
    }

    let temp_dir_path = self
      .temp_dir_path
      .clone()
      .unwrap_or_else(std::env::temp_dir);
    let deno_dir = TempDir::new_in(&temp_dir_path);
    let temp_dir = TempDir::new_in(&temp_dir_path);
    let temp_dir = if self.use_symlinked_temp_dir {
      TempDir::new_symlinked(temp_dir)
    } else {
      temp_dir
    };
    if let Some(temp_copy_dir) = &self.copy_temp_dir {
      let test_data_path = testdata_path().join(temp_copy_dir);
      let temp_copy_dir = temp_dir.path().join(temp_copy_dir);
      temp_copy_dir.create_dir_all();
      test_data_path.copy_to_recursive(&temp_copy_dir);
    }

    let deno_exe = deno_exe_path();
    println!("deno_exe path {}", deno_exe);

    let http_server_guard = if self.use_http_server {
      Some(Rc::new(http_server()))
    } else {
      None
    };

    let cwd = if self.use_temp_cwd || self.copy_temp_dir.is_some() {
      temp_dir.path().to_owned()
    } else {
      testdata_path().clone()
    };
    let cwd = match &self.cwd {
      Some(specified_cwd) => cwd.join(specified_cwd),
      None => cwd,
    };

    TestContext {
      cwd,
      deno_exe,
      envs: self.envs.clone(),
      _http_server_guard: http_server_guard,
      deno_dir,
      temp_dir,
    }
  }
}

#[derive(Clone)]
pub struct TestContext {
  deno_exe: PathRef,
  envs: HashMap<String, String>,
  cwd: PathRef,
  _http_server_guard: Option<Rc<HttpServerGuard>>,
  deno_dir: TempDir,
  temp_dir: TempDir,
}

impl Default for TestContext {
  fn default() -> Self {
    TestContextBuilder::default().build()
  }
}

impl TestContext {
  pub fn with_http_server() -> Self {
    TestContextBuilder::default().use_http_server().build()
  }

  pub fn deno_dir(&self) -> &TempDir {
    &self.deno_dir
  }

  pub fn temp_dir(&self) -> &TempDir {
    &self.temp_dir
  }

  pub fn new_command(&self) -> TestCommandBuilder {
    TestCommandBuilder::new(self.deno_dir.clone())
      .envs(self.envs.clone())
      .current_dir(&self.cwd)
  }

  pub fn new_lsp_command(&self) -> LspClientBuilder {
    LspClientBuilder::new_with_dir(self.deno_dir.clone())
      .deno_exe(&self.deno_exe)
      .set_root_dir(self.temp_dir.path().clone())
  }

  pub fn run_npm(&self, args: impl AsRef<str>) {
    self
      .new_command()
      .name("npm")
      .args(args)
      .run()
      .skip_output_check();
  }
}

/// We can't clone an stdio, so if someone clones a DenoCmd,
/// we want to set this to `Cloned` and show the user a helpful
/// panic message.
enum StdioContainer {
  Cloned,
  Inner(RefCell<Option<Stdio>>),
}

impl Clone for StdioContainer {
  fn clone(&self) -> Self {
    Self::Cloned
  }
}

impl StdioContainer {
  pub fn new(stdio: Stdio) -> Self {
    Self::Inner(RefCell::new(Some(stdio)))
  }

  pub fn take(&self) -> Stdio {
    match self {
      StdioContainer::Cloned => panic!("Cannot run a command after it was cloned. You need to reset the stdio value."),
      StdioContainer::Inner(inner) => {
        match inner.borrow_mut().take() {
          Some(value) => value,
          None => panic!("Cannot run a command that was previously run. You need to reset the stdio value between runs."),
        }
      },
    }
  }
}

#[derive(Clone)]
pub struct TestCommandBuilder {
  deno_dir: TempDir,
  stdin: Option<StdioContainer>,
  stdout: Option<StdioContainer>,
  stderr: Option<StdioContainer>,
  stdin_text: Option<String>,
  command_name: String,
  cwd: Option<PathRef>,
  envs: HashMap<String, String>,
  envs_remove: HashSet<String>,
  env_clear: bool,
  args_text: String,
  args_vec: Vec<String>,
  split_output: bool,
}

impl TestCommandBuilder {
  pub fn new(deno_dir: TempDir) -> Self {
    Self {
      deno_dir,
      stdin: None,
      stdout: None,
      stderr: None,
      stdin_text: None,
      split_output: false,
      cwd: None,
      envs: Default::default(),
      envs_remove: Default::default(),
      env_clear: false,
      command_name: "deno".to_string(),
      args_text: "".to_string(),
      args_vec: Default::default(),
    }
  }

  pub fn name(mut self, name: impl AsRef<OsStr>) -> Self {
    self.command_name = name.as_ref().to_string_lossy().to_string();
    self
  }

  pub fn args(mut self, args: impl AsRef<str>) -> Self {
    self.args_text = args.as_ref().to_string();
    self
  }

  pub fn args_vec<I, S>(mut self, args: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
  {
    self.args_vec.extend(
      args
        .into_iter()
        .map(|s| s.as_ref().to_string_lossy().to_string()),
    );
    self
  }

  pub fn arg<S>(mut self, arg: S) -> Self
  where
    S: AsRef<std::ffi::OsStr>,
  {
    self
      .args_vec
      .push(arg.as_ref().to_string_lossy().to_string());
    self
  }

  pub fn env_clear(mut self) -> Self {
    self.env_clear = true;
    self
  }

  pub fn envs<I, K, V>(self, vars: I) -> Self
  where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
  {
    let mut this = self;
    for (key, value) in vars {
      this = this.env(key, value);
    }
    this
  }

  pub fn env<K, V>(mut self, key: K, val: V) -> Self
  where
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
  {
    self.envs.insert(
      key.as_ref().to_string_lossy().to_string(),
      val.as_ref().to_string_lossy().to_string(),
    );
    self
  }

  pub fn env_remove<K>(mut self, key: K) -> Self
  where
    K: AsRef<std::ffi::OsStr>,
  {
    self
      .envs_remove
      .insert(key.as_ref().to_string_lossy().to_string());
    self
  }

  pub fn stdin<T: Into<Stdio>>(mut self, cfg: T) -> Self {
    self.stdin = Some(StdioContainer::new(cfg.into()));
    self
  }

  pub fn stdout<T: Into<Stdio>>(mut self, cfg: T) -> Self {
    self.stdout = Some(StdioContainer::new(cfg.into()));
    self
  }

  pub fn stderr<T: Into<Stdio>>(mut self, cfg: T) -> Self {
    self.stderr = Some(StdioContainer::new(cfg.into()));
    self
  }

  pub fn current_dir<P: AsRef<OsStr>>(mut self, dir: P) -> Self {
    let dir = dir.as_ref().to_string_lossy().to_string();
    self.cwd = Some(match self.cwd {
      Some(current) => current.join(dir),
      None => PathRef::new(dir),
    });
    self
  }

  pub fn stdin_piped(self) -> Self {
    self.stdin(std::process::Stdio::piped())
  }

  pub fn stdout_piped(self) -> Self {
    self.stdout(std::process::Stdio::piped())
  }

  pub fn stderr_piped(self) -> Self {
    self.stderr(std::process::Stdio::piped())
  }

  pub fn piped_output(self) -> Self {
    self.stdout_piped().stderr_piped()
  }

  pub fn stdin_text(mut self, text: impl AsRef<str>) -> Self {
    self.stdin_text = Some(text.as_ref().to_string());
    self.stdin_piped()
  }

  /// Splits the output into stdout and stderr rather than having them combined.
  pub fn split_output(mut self) -> Self {
    // Note: it was previously attempted to capture stdout & stderr separately
    // then forward the output to a combined pipe, but this was found to be
    // too racy compared to providing the same combined pipe to both.
    self.split_output = true;
    self
  }

  pub fn with_pty(&self, mut action: impl FnMut(Pty)) {
    if !Pty::is_supported() {
      return;
    }

    let args = self.build_args();
    let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let mut envs = self.build_envs();
    if !envs.contains_key("NO_COLOR") {
      // set this by default for pty tests
      envs.insert("NO_COLOR".to_string(), "1".to_string());
    }

    // note(dsherret): for some reason I need to inject the current
    // environment here for the pty tests or else I get dns errors
    if !self.env_clear {
      for (key, value) in std::env::vars() {
        envs.entry(key).or_insert(value);
      }
    }

    let cwd = self
      .cwd
      .as_ref()
      .map(PathBuf::from)
      .unwrap_or_else(|| std::env::current_dir().unwrap());
    let command_path = self.build_command_path();

    println!("command {} {}", command_path, args.join(" "));
    println!("command cwd {}", cwd.display());
    action(Pty::new(command_path.as_path(), &args, &cwd, Some(envs)))
  }

  pub fn output(&self) -> Result<std::process::Output, std::io::Error> {
    assert!(self.stdin_text.is_none(), "use spawn instead");
    self.build_command().output()
  }

  pub fn status(&self) -> Result<std::process::ExitStatus, std::io::Error> {
    assert!(self.stdin_text.is_none(), "use spawn instead");
    self.build_command().status()
  }

  pub fn spawn(&self) -> Result<DenoChild, std::io::Error> {
    let child = self.build_command().spawn()?;
    let mut child = DenoChild {
      _deno_dir: self.deno_dir.clone(),
      child,
    };

    if let Some(input) = &self.stdin_text {
      let mut p_stdin = child.stdin.take().unwrap();
      write!(p_stdin, "{input}").unwrap();
    }

    Ok(child)
  }

  pub fn spawn_with_piped_output(&self) -> DenoChild {
    self.clone().piped_output().spawn().unwrap()
  }

  pub fn run(&self) -> TestCommandOutput {
    fn read_pipe_to_string(mut pipe: os_pipe::PipeReader) -> String {
      let mut output = String::new();
      pipe.read_to_string(&mut output).unwrap();
      output
    }

    fn sanitize_output(text: String, args: &[OsString]) -> String {
      let mut text = strip_ansi_codes(&text).to_string();
      // deno test's output capturing flushes with a zero-width space in order to
      // synchronize the output pipes. Occasionally this zero width space
      // might end up in the output so strip it from the output comparison here.
      if args.first().and_then(|s| s.to_str()) == Some("test") {
        text = text.replace('\u{200B}', "");
      }
      text
    }

    let mut command = self.build_command();
    let args = command
      .get_args()
      .map(ToOwned::to_owned)
      .collect::<Vec<_>>();
    let (combined_reader, std_out_err_handle) = if self.split_output {
      let (stdout_reader, stdout_writer) = pipe().unwrap();
      let (stderr_reader, stderr_writer) = pipe().unwrap();
      command.stdout(stdout_writer);
      command.stderr(stderr_writer);
      (
        None,
        Some((
          std::thread::spawn(move || read_pipe_to_string(stdout_reader)),
          std::thread::spawn(move || read_pipe_to_string(stderr_reader)),
        )),
      )
    } else {
      let (combined_reader, combined_writer) = pipe().unwrap();
      command.stdout(combined_writer.try_clone().unwrap());
      command.stderr(combined_writer);
      (Some(combined_reader), None)
    };

    let mut process = command.spawn().expect("Failed spawning command");

    if let Some(input) = &self.stdin_text {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{input}").unwrap();
    }

    // This parent process is still holding its copies of the write ends,
    // and we have to close them before we read, otherwise the read end
    // will never report EOF. The Command object owns the writers now,
    // and dropping it closes them.
    drop(command);

    let combined = combined_reader
      .map(|pipe| sanitize_output(read_pipe_to_string(pipe), &args));

    let status = process.wait().unwrap();
    let std_out_err = std_out_err_handle.map(|(stdout, stderr)| {
      (
        sanitize_output(stdout.join().unwrap(), &args),
        sanitize_output(stderr.join().unwrap(), &args),
      )
    });
    let exit_code = status.code();
    #[cfg(unix)]
    let signal = {
      use std::os::unix::process::ExitStatusExt;
      status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;

    TestCommandOutput {
      exit_code,
      signal,
      combined,
      std_out_err,
      asserted_exit_code: RefCell::new(false),
      asserted_stdout: RefCell::new(false),
      asserted_stderr: RefCell::new(false),
      asserted_combined: RefCell::new(false),
      _deno_dir: self.deno_dir.clone(),
    }
  }

  fn build_command(&self) -> Command {
    let command_path = self.build_command_path();
    let args = self.build_args();
    println!("command {} {}", command_path, args.join(" "));
    let mut command = Command::new(command_path);
    if let Some(cwd) = &self.cwd {
      println!("command cwd {}", cwd);
      command.current_dir(cwd);
    }
    if let Some(stdin) = &self.stdin {
      command.stdin(stdin.take());
    }
    if let Some(stdout) = &self.stdout {
      command.stdout(stdout.take());
    }
    if let Some(stderr) = &self.stderr {
      command.stderr(stderr.take());
    }

    command.args(args.iter());
    if self.env_clear {
      command.env_clear();
    }
    let envs = self.build_envs();
    command.envs(envs);
    command.stdin(Stdio::piped());
    command
  }

  fn build_command_path(&self) -> PathRef {
    let command_name = if cfg!(windows) && self.command_name == "npm" {
      "npm.cmd"
    } else {
      &self.command_name
    };
    if command_name == "deno" {
      deno_exe_path()
    } else {
      PathRef::new(PathBuf::from(command_name))
    }
  }

  fn build_args(&self) -> Vec<String> {
    if self.args_vec.is_empty() {
      std::borrow::Cow::Owned(
        self
          .args_text
          .split_whitespace()
          .map(|s| s.to_string())
          .collect::<Vec<_>>(),
      )
    } else {
      assert!(
        self.args_text.is_empty(),
        "Do not provide args when providing args_vec."
      );
      std::borrow::Cow::Borrowed(&self.args_vec)
    }
    .iter()
    .map(|arg| arg.replace("$TESTDATA", &testdata_path().to_string_lossy()))
    .collect::<Vec<_>>()
  }

  fn build_envs(&self) -> HashMap<String, String> {
    let mut envs = self.envs.clone();
    if !envs.contains_key("DENO_DIR") {
      envs.insert("DENO_DIR".to_string(), self.deno_dir.path().to_string());
    }
    if !envs.contains_key("NPM_CONFIG_REGISTRY") {
      envs.insert("NPM_CONFIG_REGISTRY".to_string(), npm_registry_unset_url());
    }
    if !envs.contains_key("DENO_NO_UPDATE_CHECK") {
      envs.insert("DENO_NO_UPDATE_CHECK".to_string(), "1".to_string());
    }
    for key in &self.envs_remove {
      envs.remove(key);
    }
    envs
  }
}

pub struct DenoChild {
  // keep alive for the duration of the use of this struct
  _deno_dir: TempDir,
  child: Child,
}

impl Deref for DenoChild {
  type Target = Child;
  fn deref(&self) -> &Child {
    &self.child
  }
}

impl DerefMut for DenoChild {
  fn deref_mut(&mut self) -> &mut Child {
    &mut self.child
  }
}

impl DenoChild {
  pub fn wait_with_output(
    self,
  ) -> Result<std::process::Output, std::io::Error> {
    self.child.wait_with_output()
  }
}

pub struct TestCommandOutput {
  combined: Option<String>,
  std_out_err: Option<(String, String)>,
  exit_code: Option<i32>,
  signal: Option<i32>,
  asserted_stdout: RefCell<bool>,
  asserted_stderr: RefCell<bool>,
  asserted_combined: RefCell<bool>,
  asserted_exit_code: RefCell<bool>,
  // keep alive for the duration of the output reference
  _deno_dir: TempDir,
}

impl Drop for TestCommandOutput {
  // assert the output and exit code was asserted
  fn drop(&mut self) {
    fn panic_unasserted_output(text: &str) {
      println!("OUTPUT\n{text}\nOUTPUT");
      panic!(concat!(
        "The non-empty text of the command was not asserted. ",
        "Call `output.skip_output_check()` to skip if necessary.",
      ),);
    }

    if std::thread::panicking() {
      return;
    }

    // either the combined output needs to be asserted or both stdout and stderr
    if let Some(combined) = &self.combined {
      if !*self.asserted_combined.borrow() && !combined.is_empty() {
        panic_unasserted_output(combined);
      }
    }
    if let Some((stdout, stderr)) = &self.std_out_err {
      if !*self.asserted_stdout.borrow() && !stdout.is_empty() {
        panic_unasserted_output(stdout);
      }
      if !*self.asserted_stderr.borrow() && !stderr.is_empty() {
        panic_unasserted_output(stderr);
      }
    }

    // now ensure the exit code was asserted
    if !*self.asserted_exit_code.borrow() && self.exit_code != Some(0) {
      self.print_output();
      panic!(
        "The non-zero exit code of the command was not asserted: {:?}",
        self.exit_code,
      )
    }
  }
}

impl TestCommandOutput {
  pub fn skip_output_check(&self) -> &Self {
    *self.asserted_combined.borrow_mut() = true;
    self.skip_stdout_check();
    self.skip_stderr_check();
    self
  }

  pub fn skip_stdout_check(&self) -> &Self {
    *self.asserted_stdout.borrow_mut() = true;
    self
  }

  pub fn skip_stderr_check(&self) -> &Self {
    *self.asserted_stderr.borrow_mut() = true;
    self
  }

  pub fn skip_exit_code_check(&self) -> &Self {
    *self.asserted_exit_code.borrow_mut() = true;
    self
  }

  pub fn exit_code(&self) -> Option<i32> {
    self.skip_exit_code_check();
    self.exit_code
  }

  pub fn signal(&self) -> Option<i32> {
    self.signal
  }

  pub fn combined_output(&self) -> &str {
    self.skip_output_check();
    self
      .combined
      .as_deref()
      .expect("not available since .split_output() was called")
  }

  pub fn stdout(&self) -> &str {
    *self.asserted_stdout.borrow_mut() = true;
    self
      .std_out_err
      .as_ref()
      .map(|(stdout, _)| stdout.as_str())
      .expect("call .split_output() on the builder")
  }

  pub fn stderr(&self) -> &str {
    *self.asserted_stderr.borrow_mut() = true;
    self
      .std_out_err
      .as_ref()
      .map(|(_, stderr)| stderr.as_str())
      .expect("call .split_output() on the builder")
  }

  #[track_caller]
  pub fn assert_exit_code(&self, expected_exit_code: i32) -> &Self {
    let actual_exit_code = self.exit_code();

    if let Some(exit_code) = &actual_exit_code {
      if *exit_code != expected_exit_code {
        self.print_output();
        panic!(
          "bad exit code, expected: {:?}, actual: {:?}",
          expected_exit_code, exit_code,
        );
      }
    } else {
      self.print_output();
      if let Some(signal) = self.signal() {
        panic!(
          "process terminated by signal, expected exit code: {:?}, actual signal: {:?}",
          actual_exit_code,
          signal,
        );
      } else {
        panic!(
          "process terminated without status code on non unix platform, expected exit code: {:?}",
          actual_exit_code,
        );
      }
    }

    self
  }

  pub fn print_output(&self) {
    if let Some(combined) = &self.combined {
      println!("OUTPUT\n{combined}\nOUTPUT");
    } else if let Some((stdout, stderr)) = &self.std_out_err {
      println!("STDOUT OUTPUT\n{stdout}\nSTDOUT OUTPUT");
      println!("STDERR OUTPUT\n{stderr}\nSTDERR OUTPUT");
    }
  }

  #[track_caller]
  pub fn assert_matches_text(&self, expected_text: impl AsRef<str>) -> &Self {
    self.inner_assert_matches_text(self.combined_output(), expected_text)
  }

  #[track_caller]
  pub fn assert_matches_file(&self, file_path: impl AsRef<Path>) -> &Self {
    self.inner_assert_matches_file(self.combined_output(), file_path)
  }

  #[track_caller]
  pub fn assert_stdout_matches_text(
    &self,
    expected_text: impl AsRef<str>,
  ) -> &Self {
    self.inner_assert_matches_text(self.stdout(), expected_text)
  }

  #[track_caller]
  pub fn assert_stdout_matches_file(
    &self,
    file_path: impl AsRef<Path>,
  ) -> &Self {
    self.inner_assert_matches_file(self.stdout(), file_path)
  }

  #[track_caller]
  pub fn assert_stderr_matches_text(
    &self,
    expected_text: impl AsRef<str>,
  ) -> &Self {
    self.inner_assert_matches_text(self.stderr(), expected_text)
  }

  #[track_caller]
  pub fn assert_stderr_matches_file(
    &self,
    file_path: impl AsRef<Path>,
  ) -> &Self {
    self.inner_assert_matches_file(self.stderr(), file_path)
  }

  #[track_caller]
  fn inner_assert_matches_text(
    &self,
    actual: &str,
    expected: impl AsRef<str>,
  ) -> &Self {
    assert_wildcard_match(actual, expected.as_ref());
    self
  }

  #[track_caller]
  fn inner_assert_matches_file(
    &self,
    actual: &str,
    file_path: impl AsRef<Path>,
  ) -> &Self {
    let output_path = testdata_path().join(file_path);
    println!("output path {}", output_path);
    let expected_text = output_path.read_to_string();
    self.inner_assert_matches_text(actual, expected_text)
  }
}
