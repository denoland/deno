// Copyright 2018-2025 the Deno authors. MIT license.
use std::convert::Infallible;

use anyhow::Error;

/// For unix targets, we just replace our current process with the desired cargo process.
#[cfg(unix)]
pub fn exec_replace_inner(
  cmd: &str,
  args: &[&str],
) -> Result<Infallible, Error> {
  use std::ffi::CStr;
  use std::ffi::CString;

  let args = args
    .iter()
    .map(|arg| CString::new(*arg).unwrap())
    .collect::<Vec<_>>();
  let args: Vec<&CStr> =
    args.iter().map(|arg| arg.as_ref()).collect::<Vec<_>>();

  let err = nix::unistd::execvp(&CString::new(cmd).unwrap(), &args)
    .expect_err("Impossible");
  Err(err.into())
}

#[cfg(windows)]
pub fn exec_replace_inner(
  cmd: &str,
  args: &[&str],
) -> Result<Infallible, Error> {
  use std::os::windows::io::AsRawHandle;
  use std::process::Command;

  use win32job::ExtendedLimitInfo;
  use win32job::Job;

  // Use a job to ensure the child process's lifetime does not exceed the current process's lifetime.
  // This ensures that if the current process is terminated (e.g., via ctrl+c or task manager),
  // the child process is automatically reaped.

  // For more information about this technique, see Raymond Chen's blog post:
  // https://devblogs.microsoft.com/oldnewthing/20131209-00/?p=2433
  // Note: While our implementation is not perfect, it serves its purpose for test code.

  // In the future, we may directly obtain the main thread's handle from Rust code and use it
  // to create a suspended process that we can then resume:
  // https://github.com/rust-lang/rust/issues/96723

  // Creates a child process and assigns it to our current job.
  // A more reliable approach would be to create the child suspended and then assign it to the job.
  // For now, we create the child, create the job, and then assign both us and the child to the job.
  let mut child = Command::new(cmd).args(&args[1..]).spawn()?;

  let mut info = ExtendedLimitInfo::default();
  info.limit_kill_on_job_close();
  let job = Job::create_with_limit_info(&info)?;
  job.assign_current_process()?;
  let handle = child.as_raw_handle();
  job.assign_process(handle as _)?;

  let exit = child.wait()?;
  std::process::exit(exit.code().unwrap_or(1));
}

/// Runs a command, replacing the current process on Unix. On Windows, this function blocks and
/// exits.
///
/// In either case, the only way this function returns is if it fails to launch the child
/// process.
pub fn exec_replace(command: &str, args: &[&str]) -> Result<Infallible, Error> {
  exec_replace_inner(command, args)
}
