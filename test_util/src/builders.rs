// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::rc::Rc;

use os_pipe::pipe;

use crate::copy_dir_recursive;
use crate::deno_exe_path;
use crate::http_server;
use crate::new_deno_dir;
use crate::strip_ansi_codes;
use crate::testdata_path;
use crate::HttpServerGuard;
use crate::TempDir;

#[derive(Default)]
pub struct TestContextBuilder {
  use_http_server: bool,
  use_temp_cwd: bool,
  /// Copies the files at the specified directory in the "testdata" directory
  /// to the temp folder and runs the test from there. This is useful when
  /// the test creates files in the testdata directory (ex. a node_modules folder)
  copy_temp_dir: Option<String>,
  cwd: Option<String>,
  envs: HashMap<String, String>,
}

impl TestContextBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn use_http_server(&mut self) -> &mut Self {
    self.use_http_server = true;
    self
  }

  pub fn use_temp_cwd(&mut self) -> &mut Self {
    self.use_temp_cwd = true;
    self
  }

  /// Copies the files at the specified directory in the "testdata" directory
  /// to the temp folder and runs the test from there. This is useful when
  /// the test creates files in the testdata directory (ex. a node_modules folder)
  pub fn use_copy_temp_dir(&mut self, dir: impl AsRef<str>) {
    self.copy_temp_dir = Some(dir.as_ref().to_string());
  }

  pub fn set_cwd(&mut self, cwd: impl AsRef<str>) -> &mut Self {
    self.cwd = Some(cwd.as_ref().to_string());
    self
  }

  pub fn env(
    &mut self,
    key: impl AsRef<str>,
    value: impl AsRef<str>,
  ) -> &mut Self {
    self
      .envs
      .insert(key.as_ref().to_string(), value.as_ref().to_string());
    self
  }

  pub fn build(&self) -> TestContext {
    let deno_dir = new_deno_dir(); // keep this alive for the test
    let testdata_dir = if let Some(temp_copy_dir) = &self.copy_temp_dir {
      let test_data_path = testdata_path().join(temp_copy_dir);
      let temp_copy_dir = deno_dir.path().join(temp_copy_dir);
      std::fs::create_dir_all(&temp_copy_dir).unwrap();
      copy_dir_recursive(&test_data_path, &temp_copy_dir).unwrap();
      deno_dir.path().to_owned()
    } else {
      testdata_path()
    };

    let deno_exe = deno_exe_path();
    println!("deno_exe path {}", deno_exe.display());

    let http_server_guard = if self.use_http_server {
      Some(Rc::new(http_server()))
    } else {
      None
    };

    TestContext {
      cwd: self.cwd.clone(),
      envs: self.envs.clone(),
      use_temp_cwd: self.use_temp_cwd,
      _http_server_guard: http_server_guard,
      deno_dir,
      testdata_dir,
    }
  }
}

#[derive(Clone)]
pub struct TestContext {
  envs: HashMap<String, String>,
  use_temp_cwd: bool,
  cwd: Option<String>,
  _http_server_guard: Option<Rc<HttpServerGuard>>,
  deno_dir: TempDir,
  testdata_dir: PathBuf,
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

  pub fn testdata_path(&self) -> &PathBuf {
    &self.testdata_dir
  }

  pub fn deno_dir(&self) -> &TempDir {
    &self.deno_dir
  }

  pub fn new_command(&self) -> TestCommandBuilder {
    TestCommandBuilder {
      command_name: Default::default(),
      args: Default::default(),
      args_vec: Default::default(),
      stdin: Default::default(),
      envs: Default::default(),
      env_clear: Default::default(),
      cwd: Default::default(),
      context: self.clone(),
    }
  }
}

pub struct TestCommandBuilder {
  command_name: Option<String>,
  args: String,
  args_vec: Vec<String>,
  stdin: Option<String>,
  envs: HashMap<String, String>,
  env_clear: bool,
  cwd: Option<String>,
  context: TestContext,
}

impl TestCommandBuilder {
  pub fn command_name(&mut self, name: impl AsRef<str>) -> &mut Self {
    self.command_name = Some(name.as_ref().to_string());
    self
  }

  pub fn args(&mut self, text: impl AsRef<str>) -> &mut Self {
    self.args = text.as_ref().to_string();
    self
  }

  pub fn args_vec(&mut self, args: Vec<String>) -> &mut Self {
    self.args_vec = args;
    self
  }

  pub fn stdin(&mut self, text: impl AsRef<str>) -> &mut Self {
    self.stdin = Some(text.as_ref().to_string());
    self
  }

  pub fn env(
    &mut self,
    key: impl AsRef<str>,
    value: impl AsRef<str>,
  ) -> &mut Self {
    self
      .envs
      .insert(key.as_ref().to_string(), value.as_ref().to_string());
    self
  }

  pub fn env_clear(&mut self) -> &mut Self {
    self.env_clear = true;
    self
  }

  pub fn cwd(&mut self, cwd: impl AsRef<str>) -> &mut Self {
    self.cwd = Some(cwd.as_ref().to_string());
    self
  }

  pub fn run(&self) -> TestCommandOutput {
    let cwd = self.cwd.as_ref().or(self.context.cwd.as_ref());
    let cwd = if self.context.use_temp_cwd {
      assert!(cwd.is_none());
      self.context.deno_dir.path().to_owned()
    } else if let Some(cwd_) = cwd {
      self.context.testdata_dir.join(cwd_)
    } else {
      self.context.testdata_dir.clone()
    };
    let args = if self.args_vec.is_empty() {
      std::borrow::Cow::Owned(
        self
          .args
          .split_whitespace()
          .map(|s| s.to_string())
          .collect::<Vec<_>>(),
      )
    } else {
      assert!(
        self.args.is_empty(),
        "Do not provide args when providing args_vec."
      );
      std::borrow::Cow::Borrowed(&self.args_vec)
    }
    .iter()
    .map(|arg| {
      arg.replace("$TESTDATA", &self.context.testdata_dir.to_string_lossy())
    })
    .collect::<Vec<_>>();
    let (mut reader, writer) = pipe().unwrap();
    let command_name = self
      .command_name
      .as_ref()
      .cloned()
      .unwrap_or("deno".to_string());
    let mut command = if command_name == "deno" {
      Command::new(deno_exe_path())
    } else {
      Command::new(&command_name)
    };
    command.env("DENO_DIR", self.context.deno_dir.path());

    println!("command {} {}", command_name, args.join(" "));
    println!("command cwd {:?}", &cwd);
    command.args(args.iter());
    if self.env_clear {
      command.env_clear();
    }
    command.envs({
      let mut envs = self.context.envs.clone();
      for (key, value) in &self.envs {
        envs.insert(key.to_string(), value.to_string());
      }
      envs
    });
    command.current_dir(cwd);
    command.stdin(Stdio::piped());
    let writer_clone = writer.try_clone().unwrap();
    command.stderr(writer_clone);
    command.stdout(writer);

    let mut process = command.spawn().unwrap();

    if let Some(input) = &self.stdin {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{input}").unwrap();
    }

    // This parent process is still holding its copies of the write ends,
    // and we have to close them before we read, otherwise the read end
    // will never report EOF. The Command object owns the writers now,
    // and dropping it closes them.
    drop(command);

    let mut actual = String::new();
    reader.read_to_string(&mut actual).unwrap();

    let status = process.wait().expect("failed to finish process");
    let exit_code = status.code();
    #[cfg(unix)]
    let signal = {
      use std::os::unix::process::ExitStatusExt;
      status.signal()
    };
    #[cfg(not(unix))]
    let signal = None;

    actual = strip_ansi_codes(&actual).to_string();

    // deno test's output capturing flushes with a zero-width space in order to
    // synchronize the output pipes. Occassionally this zero width space
    // might end up in the output so strip it from the output comparison here.
    if args.first().map(|s| s.as_str()) == Some("test") {
      actual = actual.replace('\u{200B}', "");
    }

    TestCommandOutput {
      exit_code,
      signal,
      text: actual,
      testdata_dir: self.context.testdata_dir.clone(),
      asserted_exit_code: RefCell::new(false),
      asserted_text: RefCell::new(false),
      _test_context: self.context.clone(),
    }
  }
}

pub struct TestCommandOutput {
  text: String,
  exit_code: Option<i32>,
  signal: Option<i32>,
  testdata_dir: PathBuf,
  asserted_text: RefCell<bool>,
  asserted_exit_code: RefCell<bool>,
  // keep alive for the duration of the output reference
  _test_context: TestContext,
}

impl Drop for TestCommandOutput {
  fn drop(&mut self) {
    if std::thread::panicking() {
      return;
    }
    // force the caller to assert these
    if !*self.asserted_exit_code.borrow() && self.exit_code != Some(0) {
      panic!(
        "The non-zero exit code of the command was not asserted: {:?}.",
        self.exit_code
      )
    }
    if !*self.asserted_text.borrow() && !self.text.is_empty() {
      println!("OUTPUT\n{}\nOUTPUT", self.text);
      panic!(concat!(
        "The non-empty text of the command was not asserted. ",
        "Call `output.skip_output_check()` to skip if necessary.",
      ));
    }
  }
}

impl TestCommandOutput {
  pub fn testdata_dir(&self) -> &PathBuf {
    &self.testdata_dir
  }

  pub fn skip_output_check(&self) {
    *self.asserted_text.borrow_mut() = true;
  }

  pub fn skip_exit_code_check(&self) {
    *self.asserted_exit_code.borrow_mut() = true;
  }

  pub fn exit_code(&self) -> Option<i32> {
    self.skip_exit_code_check();
    self.exit_code
  }

  pub fn signal(&self) -> Option<i32> {
    self.signal
  }

  pub fn text(&self) -> &str {
    self.skip_output_check();
    &self.text
  }
}
