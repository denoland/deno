// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::process::Child;
use std::process::ExitStatus;

use anyhow::bail;

pub fn wait_on_child_with_timeout(
  mut child: Child,
  timeout: std::time::Duration,
) -> Result<ExitStatus, anyhow::Error> {
  let (tx, rx) = std::sync::mpsc::channel();
  let pid = child.id();

  std::thread::spawn(move || {
    let status = child.wait().unwrap();
    _ = tx.send(status);
  });

  match rx.recv_timeout(timeout) {
    Ok(status) => Ok(status),
    Err(_) => {
      kill(pid as i32)?;
      bail!(
        "Child process timed out after {}s",
        timeout.as_secs_f32().ceil() as u64
      );
    }
  }
}

#[cfg(unix)]
pub fn kill(pid: i32) -> Result<(), anyhow::Error> {
  use nix::sys::signal::kill as unix_kill;
  use nix::sys::signal::Signal;
  use nix::unistd::Pid;
  let sig = Signal::SIGTERM;
  Ok(unix_kill(Pid::from_raw(pid), Some(sig))?)
}

#[cfg(not(unix))]
pub fn kill(pid: i32) -> Result<(), anyhow::Error> {
  use std::io::Error;
  use std::io::ErrorKind::NotFound;
  use winapi::shared::minwindef::DWORD;
  use winapi::shared::minwindef::FALSE;
  use winapi::shared::minwindef::TRUE;
  use winapi::shared::winerror::ERROR_INVALID_PARAMETER;
  use winapi::um::errhandlingapi::GetLastError;
  use winapi::um::handleapi::CloseHandle;
  use winapi::um::processthreadsapi::OpenProcess;
  use winapi::um::processthreadsapi::TerminateProcess;
  use winapi::um::winnt::PROCESS_TERMINATE;

  if pid <= 0 {
    bail!("Invalid pid: {}", pid);
  }
  let handle =
      // SAFETY: winapi call
      unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, pid as DWORD) };

  if handle.is_null() {
    // SAFETY: winapi call
    let err = match unsafe { GetLastError() } {
      ERROR_INVALID_PARAMETER => Error::from(NotFound), // Invalid `pid`.
      errno => Error::from_raw_os_error(errno as i32),
    };
    Err(err.into())
  } else {
    // SAFETY: winapi calls
    unsafe {
      let is_terminated = TerminateProcess(handle, 1);
      CloseHandle(handle);
      match is_terminated {
        FALSE => Err(Error::last_os_error().into()),
        TRUE => Ok(()),
        _ => unreachable!(),
      }
    }
  }
}
