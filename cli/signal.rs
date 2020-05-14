// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::op_error::OpError;

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> Result<(), OpError> {
  use nix::sys::signal::{kill as unix_kill, Signal};
  use nix::unistd::Pid;
  use std::convert::TryFrom;
  let sig = Signal::try_from(signo)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(OpError::from)
}

#[cfg(not(unix))]
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
use winapi::um::winnt::PROCESS_TERMINATE;
pub fn kill(pid: i32, signal: i32) -> Result<(), OpError> {
  unsafe {
    let handle = OpenProcess(PROCESS_TERMINATE, 0, pid as u32);
    if handle.is_null() {
      let m = format!("failed to open process : {}", pid);
      return Err(OpError::other(m));
    }
    if TerminateProcess(handle, signal as u32) == 0 {
      let m = format!("failed to terminate process : {}", pid);
      return Err(OpError::other(m));
    }
    if CloseHandle(handle) == 0 {
      let m = format!("failed to close handle process : {}", pid);
      return Err(OpError::other(m));
    }
  }
  Ok(())
}
