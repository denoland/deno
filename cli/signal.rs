// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;

#[cfg(not(unix))]
use deno_core::error::last_os_error;
#[cfg(not(unix))]
use deno_core::error::type_error;

#[cfg(not(unix))]
const SIGINT: i32 = 2;
#[cfg(not(unix))]
const SIGKILL: i32 = 9;
#[cfg(not(unix))]
const SIGTERM: i32 = 15;

#[cfg(not(unix))]
use winapi::{
  shared::minwindef::DWORD,
  um::{
    handleapi::CloseHandle,
    processthreadsapi::{OpenProcess, TerminateProcess},
    winnt::PROCESS_TERMINATE,
  },
};

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> Result<(), AnyError> {
  use nix::sys::signal::{kill as unix_kill, Signal};
  use nix::unistd::Pid;
  use std::convert::TryFrom;
  let sig = Signal::try_from(signo)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(AnyError::from)
}

#[cfg(not(unix))]
pub fn kill(pid: i32, signal: i32) -> Result<(), AnyError> {
  match signal {
    SIGINT | SIGKILL | SIGTERM => {
      if pid <= 0 {
        return Err(type_error("unsupported pid"));
      }
      unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid as DWORD);
        if handle.is_null() {
          return Err(last_os_error());
        }
        if TerminateProcess(handle, 1) == 0 {
          CloseHandle(handle);
          return Err(last_os_error());
        }
        if CloseHandle(handle) == 0 {
          return Err(last_os_error());
        }
      }
    }
    _ => {
      return Err(type_error("unsupported signal"));
    }
  }
  Ok(())
}
