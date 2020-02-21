// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::DenoError;

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> Result<(), DenoError> {
  use nix::sys::signal::{kill as unix_kill, Signal};
  use nix::unistd::Pid;
  let sig = Signal::from_c_int(signo)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(DenoError::from)
}

#[cfg(not(unix))]
pub fn kill(_pid: i32, _signal: i32) -> Result<(), ErrBox> {
  // NOOP
  // TODO: implement this for windows
  Ok(())
}
