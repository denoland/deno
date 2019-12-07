// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use deno_core::ErrBox;
#[cfg(unix)]
use deno_core::Resource;

#[cfg(unix)]
use tokio::signal::unix::Signal;

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> Result<(), ErrBox> {
  use nix::sys::signal::{kill as unix_kill, Signal};
  use nix::unistd::Pid;
  let sig = Signal::from_c_int(signo)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(ErrBox::from)
}

#[cfg(not(unix))]
pub fn kill(_pid: i32, _signal: i32) -> Result<(), ErrBox> {
  // NOOP
  // TODO: implement this for windows
  Ok(())
}

#[cfg(unix)]
pub struct SignalStreamResource(pub Signal, pub Option<std::task::Waker>);

#[cfg(unix)]
impl Resource for SignalStreamResource {}
