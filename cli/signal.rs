// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#[cfg(not(unix))]
use crate::op_error::io_to_errbox;
#[cfg(unix)]
use crate::op_error::nix_to_errbox;
use deno_core::ErrBox;

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
pub fn kill(pid: i32, signo: i32) -> Result<(), ErrBox> {
  use nix::sys::signal::{kill as unix_kill, Signal};
  use nix::unistd::Pid;
  use std::convert::TryFrom;
  let sig = Signal::try_from(signo).map_err(nix_to_errbox)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(nix_to_errbox)
}

#[cfg(not(unix))]
pub fn kill(pid: i32, signal: i32) -> Result<(), ErrBox> {
  match signal {
    SIGINT | SIGKILL | SIGTERM => {
      if pid <= 0 {
        return Err(ErrBox::type_error("unsupported pid".to_string()));
      }
      unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid as DWORD);
        if handle.is_null() {
          return Err(io_to_errbox(std::io::Error::last_os_error()));
        }
        if TerminateProcess(handle, 1) == 0 {
          CloseHandle(handle);
          return Err(io_to_errbox(std::io::Error::last_os_error()));
        }
        if CloseHandle(handle) == 0 {
          return Err(io_to_errbox(std::io::Error::last_os_error()));
        }
      }
    }
    _ => {
      return Err(ErrBox::type_error("unsupported signal".to_string()));
    }
  }
  Ok(())
}
