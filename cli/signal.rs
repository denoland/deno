// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::op_error::OpError;

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> Result<(), OpError> {
  use std::convert::TryFrom;
  let sig = nix::sys::signal::Signal::try_from(signo)?;
  nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), Option::Some(sig))
    .map_err(OpError::from)
}

#[cfg(not(unix))]
pub fn kill(_pid: i32, _signal: i32) -> Result<(), OpError> {
  // NOOP
  // TODO: implement this for windows
  Ok(())
}
