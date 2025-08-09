// Copyright 2018-2025 the Deno authors. MIT license.

// Parts adapted from tokio, license below
// MIT License
//
// Copyright (c) Tokio Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#![allow(clippy::undocumented_unsafe_blocks)]
#![cfg(windows)]

#[cfg(test)]
mod tests;

mod process;
mod process_stdio;

mod anon_pipe;
mod env;
mod uv_error;
mod widestr;

use std::borrow::Cow;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::io::Read;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::IntoRawHandle;
use std::os::windows::process::ExitStatusExt;
use std::os::windows::raw::HANDLE;
use std::path::Path;
use std::pin::Pin;
use std::process::ChildStderr;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::ExitStatus;
use std::process::Output;
use std::task::Context;
use std::task::Poll;

use anon_pipe::read2;
use env::CommandEnv;
pub use process::process_kill;
pub use process_stdio::disable_stdio_inheritance;

use crate::process::*;
use crate::process_stdio::*;

#[derive(Debug)]
pub enum Stdio {
  Inherit,
  Pipe,
  Null,
  RawHandle(HANDLE),
}

impl From<Stdio> for std::process::Stdio {
  fn from(stdio: Stdio) -> Self {
    match stdio {
      Stdio::Inherit => std::process::Stdio::inherit(),
      Stdio::Pipe => std::process::Stdio::piped(),
      Stdio::Null => std::process::Stdio::null(),
      Stdio::RawHandle(handle) => unsafe {
        std::process::Stdio::from_raw_handle(handle)
      },
    }
  }
}

impl Stdio {
  pub fn inherit() -> Self {
    Stdio::Inherit
  }

  pub fn piped() -> Self {
    Stdio::Pipe
  }

  pub fn null() -> Self {
    Stdio::Null
  }
}

impl<T: IntoRawHandle> From<T> for Stdio {
  fn from(value: T) -> Self {
    Stdio::RawHandle(value.into_raw_handle())
  }
}

pub struct Child {
  inner: FusedChild,
  pub stdin: Option<ChildStdin>,
  pub stdout: Option<ChildStdout>,
  pub stderr: Option<ChildStderr>,
}

/// An interface for killing a running process.
/// Copied from https://github.com/tokio-rs/tokio/blob/ab8d7b82a1252b41dc072f641befb6d2afcb3373/tokio/src/process/kill.rs
pub(crate) trait Kill {
  /// Forcefully kills the process.
  fn kill(&mut self) -> io::Result<()>;
}

impl<T: Kill> Kill for &mut T {
  fn kill(&mut self) -> io::Result<()> {
    (**self).kill()
  }
}

/// A drop guard which can ensure the child process is killed on drop if specified.
///
/// From https://github.com/tokio-rs/tokio/blob/ab8d7b82a1252b41dc072f641befb6d2afcb3373/tokio/src/process/mod.rs
#[derive(Debug)]
struct ChildDropGuard<T: Kill> {
  inner: T,
  kill_on_drop: bool,
}

impl<T: Kill> Kill for ChildDropGuard<T> {
  fn kill(&mut self) -> io::Result<()> {
    let ret = self.inner.kill();

    if ret.is_ok() {
      self.kill_on_drop = false;
    }

    ret
  }
}

impl<T: Kill> Drop for ChildDropGuard<T> {
  fn drop(&mut self) {
    if self.kill_on_drop {
      drop(self.kill());
    }
  }
}

// copied from https://github.com/tokio-rs/tokio/blob/ab8d7b82a1252b41dc072f641befb6d2afcb3373/tokio/src/process/mod.rs
impl<T, E, F> Future for ChildDropGuard<F>
where
  F: Future<Output = Result<T, E>> + Kill + Unpin,
{
  type Output = Result<T, E>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    let ret = Pin::new(&mut self.inner).poll(cx);

    if let Poll::Ready(Ok(_)) = ret {
      // Avoid the overhead of trying to kill a reaped process
      self.kill_on_drop = false;
    }

    ret
  }
}

/// Keeps track of the exit status of a child process without worrying about
/// polling the underlying futures even after they have completed.
///
/// From https://github.com/tokio-rs/tokio/blob/ab8d7b82a1252b41dc072f641befb6d2afcb3373/tokio/src/process/mod.rs
#[derive(Debug)]
enum FusedChild {
  Child(ChildDropGuard<ChildProcess>),
  Done(i32),
}

impl Child {
  pub fn id(&self) -> Option<u32> {
    match &self.inner {
      FusedChild::Child(child) => Some(child.inner.pid() as u32),
      FusedChild::Done(_) => None,
    }
  }

  pub fn wait_blocking(&mut self) -> Result<ExitStatus, std::io::Error> {
    drop(self.stdin.take());
    match &mut self.inner {
      FusedChild::Child(child) => child
        .inner
        .wait()
        .map(|code| ExitStatus::from_raw(code as u32)),
      FusedChild::Done(code) => Ok(ExitStatus::from_raw(*code as u32)),
    }
  }

  pub async fn wait(&mut self) -> io::Result<ExitStatus> {
    // Ensure stdin is closed so the child isn't stuck waiting on
    // input while the parent is waiting for it to exit.
    drop(self.stdin.take());

    match &mut self.inner {
      FusedChild::Done(exit) => Ok(ExitStatus::from_raw(*exit as u32)),
      FusedChild::Child(child) => {
        let ret = child.await;

        if let Ok(exit) = ret {
          self.inner = FusedChild::Done(exit);
        }

        ret.map(|code| ExitStatus::from_raw(code as u32))
      }
    }
  }

  pub fn try_wait(&mut self) -> Result<Option<i32>, std::io::Error> {
    match &mut self.inner {
      FusedChild::Done(exit) => Ok(Some(*exit)),
      FusedChild::Child(child) => child.inner.try_wait(),
    }
  }

  // from std
  pub fn wait_with_output(&mut self) -> io::Result<Output> {
    drop(self.stdin.take());

    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    match (self.stdout.take(), self.stderr.take()) {
      (None, None) => {}
      (Some(mut out), None) => {
        let res = out.read_to_end(&mut stdout);
        res.unwrap();
      }
      (None, Some(mut err)) => {
        let res = err.read_to_end(&mut stderr);
        res.unwrap();
      }
      (Some(out), Some(err)) => {
        let res = read2(
          unsafe {
            crate::anon_pipe::AnonPipe::from_raw_handle(out.into_raw_handle())
          },
          &mut stdout,
          unsafe {
            crate::anon_pipe::AnonPipe::from_raw_handle(err.into_raw_handle())
          },
          &mut stderr,
        );
        res.unwrap();
      }
    }

    let status = self.wait_blocking()?;
    Ok(Output {
      status,
      stdout,
      stderr,
    })
  }
}

pub struct Command {
  program: OsString,
  args: Vec<OsString>,
  envs: CommandEnv,
  detached: bool,
  cwd: Option<OsString>,
  stdin: Stdio,
  stdout: Stdio,
  stderr: Stdio,
  extra_handles: Vec<Option<HANDLE>>,
  kill_on_drop: bool,
  verbatim_arguments: bool,
}

impl Command {
  pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
    Self {
      program: program.as_ref().to_os_string(),
      args: vec![program.as_ref().to_os_string()],
      envs: CommandEnv::default(),
      detached: false,
      cwd: None,
      stdin: Stdio::Inherit,
      stdout: Stdio::Inherit,
      stderr: Stdio::Inherit,
      extra_handles: vec![],
      kill_on_drop: false,
      verbatim_arguments: false,
    }
  }

  pub fn verbatim_arguments(&mut self, verbatim: bool) -> &mut Self {
    self.verbatim_arguments = verbatim;
    self
  }

  pub fn get_current_dir(&self) -> Option<&Path> {
    self.cwd.as_deref().map(Path::new)
  }

  pub fn current_dir<S: AsRef<Path>>(&mut self, cwd: S) -> &mut Self {
    self.cwd = Some(cwd.as_ref().to_path_buf().into_os_string());
    self
  }

  pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
    self.args.push(arg.as_ref().to_os_string());
    self
  }

  pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(
    &mut self,
    args: I,
  ) -> &mut Self {
    self
      .args
      .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
    self
  }

  pub fn env<S: AsRef<OsStr>, T: AsRef<OsStr>>(
    &mut self,
    key: S,
    value: T,
  ) -> &mut Self {
    self.envs.set(key.as_ref(), value.as_ref());
    self
  }

  pub fn get_program(&self) -> &OsStr {
    self.program.as_os_str()
  }

  pub fn get_args(&self) -> impl Iterator<Item = &OsStr> {
    self.args.iter().skip(1).map(|a| a.as_os_str())
  }

  pub fn envs<
    I: IntoIterator<Item = (S, T)>,
    S: AsRef<OsStr>,
    T: AsRef<OsStr>,
  >(
    &mut self,
    envs: I,
  ) -> &mut Self {
    for (k, v) in envs {
      self.envs.set(k.as_ref(), v.as_ref());
    }
    self
  }

  pub fn kill_on_drop(&mut self, kill_on_drop: bool) -> &mut Self {
    self.kill_on_drop = kill_on_drop;
    self
  }

  pub fn detached(&mut self) -> &mut Self {
    self.detached = true;
    self.kill_on_drop = false;
    self
  }

  pub fn env_clear(&mut self) -> &mut Self {
    self.envs.clear();
    self
  }

  pub fn stdin(&mut self, stdin: Stdio) -> &mut Self {
    self.stdin = stdin;
    self
  }

  pub fn stdout(&mut self, stdout: Stdio) -> &mut Self {
    self.stdout = stdout;
    self
  }

  pub fn stderr(&mut self, stderr: Stdio) -> &mut Self {
    self.stderr = stderr;
    self
  }

  pub fn extra_handle(&mut self, handle: Option<HANDLE>) -> &mut Self {
    self.extra_handles.push(handle);
    self
  }

  pub fn spawn(&mut self) -> Result<Child, std::io::Error> {
    let mut flags = 0;
    if self.detached {
      flags |= uv_process_flags::Detached;
    }
    if self.verbatim_arguments {
      flags |= uv_process_flags::WindowsVerbatimArguments;
    }

    let (stdin, child_stdin) = match self.stdin {
      Stdio::Pipe => {
        let pipes = crate::anon_pipe::anon_pipe(false, true)?;
        let child_stdin_handle = pipes.ours.into_handle();
        let stdin_handle = pipes.theirs.into_handle().into_raw_handle();

        (
          StdioContainer::RawHandle(stdin_handle),
          Some(ChildStdin::from(child_stdin_handle)),
        )
      }
      Stdio::Null => (StdioContainer::Ignore, None),
      Stdio::Inherit => (StdioContainer::InheritFd(0), None),
      Stdio::RawHandle(handle) => (StdioContainer::RawHandle(handle), None),
    };
    let (stdout, child_stdout) = match self.stdout {
      Stdio::Pipe => {
        let pipes = crate::anon_pipe::anon_pipe(true, true)?;
        let child_stdout_handle = pipes.ours.into_handle();
        let stdout_handle = pipes.theirs.into_handle().into_raw_handle();

        (
          StdioContainer::RawHandle(stdout_handle),
          Some(ChildStdout::from(child_stdout_handle)),
        )
      }
      Stdio::Null => (StdioContainer::Ignore, None),
      Stdio::Inherit => (StdioContainer::InheritFd(1), None),
      Stdio::RawHandle(handle) => (StdioContainer::RawHandle(handle), None),
    };
    let (stderr, child_stderr) = match self.stderr {
      Stdio::Pipe => {
        let pipes = crate::anon_pipe::anon_pipe(true, true)?;
        let child_stderr_handle = pipes.ours.into_handle();
        let stderr_handle = pipes.theirs.into_handle().into_raw_handle();

        (
          StdioContainer::RawHandle(stderr_handle),
          Some(ChildStderr::from(child_stderr_handle)),
        )
      }
      Stdio::Null => (StdioContainer::Ignore, None),
      Stdio::Inherit => (StdioContainer::InheritFd(2), None),
      Stdio::RawHandle(handle) => (StdioContainer::RawHandle(handle), None),
    };

    let mut stdio = Vec::with_capacity(3 + self.extra_handles.len());
    stdio.extend([stdin, stdout, stderr]);
    stdio.extend(self.extra_handles.iter().map(|h| {
      h.map(StdioContainer::RawHandle)
        .unwrap_or(StdioContainer::Ignore)
    }));

    crate::process::spawn(&SpawnOptions {
      flags,
      file: Cow::Borrowed(&self.program),
      args: self
        .args
        .iter()
        .map(|a| Cow::Borrowed(a.as_os_str()))
        .collect(),
      env: &self.envs,
      cwd: self.cwd.as_deref().map(Cow::Borrowed),
      stdio,
    })
    .map_err(|err| std::io::Error::other(err.to_string()))
    .map(|process| Child {
      inner: FusedChild::Child(ChildDropGuard {
        inner: process,
        kill_on_drop: self.kill_on_drop,
      }),
      stdin: child_stdin,
      stdout: child_stdout,
      stderr: child_stderr,
    })
  }
}
