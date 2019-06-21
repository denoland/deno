#[cfg(unix)]
use nix::sys::signal::{kill as unix_kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

use crate::deno_error::DenoResult;

#[cfg(unix)]
pub fn kill(pid: i32, signo: i32) -> DenoResult<()> {
  use crate::deno_error::DenoError;
  let sig = Signal::from_c_int(signo)?;
  unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(DenoError::from)
}

#[cfg(not(unix))]
pub fn kill(_pid: i32, _signal: i32) -> DenoResult<()> {
  // NOOP
  // TODO: implement this for windows
  Ok(())
}
