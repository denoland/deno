use deno_core::ErrBox;

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
