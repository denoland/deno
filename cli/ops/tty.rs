use super::dispatch_json::JsonOp;
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::deno_error::other_error;
use crate::ops::json_op;
use crate::state::State;
use atty;
use deno_core::*;
#[cfg(unix)]
use nix::sys::termios;
use serde_derive::Deserialize;
#[cfg(unix)]
use serde_derive::Serialize;
use serde_json::Value;

#[cfg(windows)]
use winapi::shared::minwindef::DWORD;
#[cfg(windows)]
use winapi::um::wincon;
#[cfg(windows)]
const RAW_MODE_MASK: DWORD = wincon::ENABLE_LINE_INPUT
  | wincon::ENABLE_ECHO_INPUT
  | wincon::ENABLE_PROCESSED_INPUT;
#[cfg(windows)]
fn get_windows_handle(
  f: &std::fs::File,
) -> Result<std::os::windows::io::RawHandle, ErrBox> {
  use std::os::windows::io::AsRawHandle;
  use winapi::um::handleapi;

  let handle = f.as_raw_handle();
  if handle == handleapi::INVALID_HANDLE_VALUE {
    return Err(ErrBox::from(std::io::Error::last_os_error()));
  } else if handle.is_null() {
    return Err(ErrBox::from(other_error("null handle".to_owned())));
  }
  Ok(handle)
}

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("set_raw", s.core_op(json_op(s.stateful_op(op_set_raw))));
  i.register_op("isatty", s.core_op(json_op(s.stateful_op(op_isatty))));
}

#[cfg(windows)]
macro_rules! wincheck {
  ($funcall:expr) => {{
    let rc = unsafe { $funcall };
    if rc == 0 {
      Err(ErrBox::from(std::io::Error::last_os_error()))?;
    }
    rc
  }};
}

// libc::termios cannot be serialized.
// Create a similar one for our use.
#[cfg(unix)]
#[derive(Serialize, Deserialize)]
struct SerializedTermios {
  iflags: libc::tcflag_t,
  oflags: libc::tcflag_t,
  cflags: libc::tcflag_t,
  lflags: libc::tcflag_t,
  cc: [libc::cc_t; libc::NCCS],
}

#[cfg(unix)]
impl From<termios::Termios> for SerializedTermios {
  fn from(t: termios::Termios) -> Self {
    Self {
      iflags: t.input_flags.bits(),
      oflags: t.output_flags.bits(),
      cflags: t.control_flags.bits(),
      lflags: t.local_flags.bits(),
      cc: t.control_chars,
    }
  }
}

#[cfg(unix)]
impl Into<termios::Termios> for SerializedTermios {
  fn into(self) -> termios::Termios {
    let mut t = unsafe { termios::Termios::default_uninit() };
    t.input_flags = termios::InputFlags::from_bits_truncate(self.iflags);
    t.output_flags = termios::OutputFlags::from_bits_truncate(self.oflags);
    t.control_flags = termios::ControlFlags::from_bits_truncate(self.cflags);
    t.local_flags = termios::LocalFlags::from_bits_truncate(self.lflags);
    t.control_chars = self.cc;
    t
  }
}

#[derive(Deserialize)]
struct SetRawArgs {
  rid: u32,
  raw: bool,
  #[cfg(unix)]
  restore: Option<String>, // Only used for *nix
                           // Saved as string in case of u64 problem in JS
}

pub fn op_set_raw(
  state_: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetRawArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let is_raw = args.raw;

  let state = state_.borrow_mut();
  let resource = state.resource_table.get::<StreamResource>(rid);
  if resource.is_none() {
    return Err(bad_resource());
  }

  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/windows.rs
  // and https://github.com/kkawakam/rustyline/blob/master/src/tty/unix.rs
  // and https://github.com/crossterm-rs/crossterm/blob/e35d4d2c1cc4c919e36d242e014af75f6127ab50/src/terminal/sys/windows.rs
  // Copyright (c) 2015 Katsu Kawakami & Rustyline authors. MIT license.
  // Copyright (c) 2019 Timon. MIT license.
  #[cfg(windows)]
  {
    use std::os::windows::io::AsRawHandle;
    use winapi::um::{consoleapi, handleapi};

    // For now, only stdin.
    let handle = match resource.unwrap() {
      StreamResource::Stdin(_) => std::io::stdin().as_raw_handle(),
      StreamResource::FsFile(f) => {
        let tokio_file = futures::executor::block_on(f.try_clone())?;
        let std_file = futures::executor::block_on(tokio_file.into_std());
        std_file.as_raw_handle()
      }
      _ => {
        return Err(other_error("Not implemented".to_owned()));
      }
    };

    if handle == handleapi::INVALID_HANDLE_VALUE {
      return Err(ErrBox::from(std::io::Error::last_os_error()));
    } else if handle.is_null() {
      return Err(ErrBox::from(other_error("null handle".to_owned())));
    }
    let mut original_mode: DWORD = 0;
    wincheck!(consoleapi::GetConsoleMode(handle, &mut original_mode));
    let new_mode = if is_raw {
      original_mode & !RAW_MODE_MASK
    } else {
      original_mode | RAW_MODE_MASK
    };
    wincheck!(consoleapi::SetConsoleMode(handle, new_mode));

    Ok(JsonOp::Sync(json!({})))
  }
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    let raw_fd = match resource.unwrap() {
      StreamResource::Stdin(_) => std::io::stdin().as_raw_fd(),
      StreamResource::FsFile(f) => {
        let tokio_file = futures::executor::block_on(f.try_clone())?;
        let std_file = futures::executor::block_on(tokio_file.into_std());
        std_file.as_raw_fd()
      }
      _ => {
        return Err(other_error("Not implemented".to_owned()));
      }
    };

    if is_raw {
      let original_mode = termios::tcgetattr(raw_fd)?;
      let mut raw = original_mode.clone();

      raw.input_flags &= !(termios::InputFlags::BRKINT
        | termios::InputFlags::ICRNL
        | termios::InputFlags::INPCK
        | termios::InputFlags::ISTRIP
        | termios::InputFlags::IXON);

      raw.control_flags |= termios::ControlFlags::CS8;

      raw.local_flags &= !(termios::LocalFlags::ECHO
        | termios::LocalFlags::ICANON
        | termios::LocalFlags::IEXTEN
        | termios::LocalFlags::ISIG);
      raw.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 1;
      raw.control_chars[termios::SpecialCharacterIndices::VTIME as usize] = 0;
      termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &raw)?;
      Ok(JsonOp::Sync(json!({
        "restore":
          serde_json::to_string(&SerializedTermios::from(original_mode)).unwrap(),
      })))
    } else {
      // Restore old mode.
      if args.restore.is_none() {
        return Err(other_error("no termios to restore".to_owned()));
      }
      let old_termios =
        serde_json::from_str::<SerializedTermios>(&args.restore.unwrap());
      if old_termios.is_err() {
        return Err(other_error("bad termios to restore".to_owned()));
      }
      let old_termios = old_termios.unwrap();
      termios::tcsetattr(
        raw_fd,
        termios::SetArg::TCSADRAIN,
        &old_termios.into(),
      )?;
      Ok(JsonOp::Sync(json!({})))
    }
  }
}

#[derive(Deserialize)]
struct IsattyArgs {
  rid: u32,
}

pub fn op_isatty(
  state_: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: IsattyArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let state = state_.borrow_mut();
  if !state.resource_table.has(rid) {
    return Err(bad_resource());
  }

  let resource = state.resource_table.get::<StreamResource>(rid);
  if resource.is_none() {
    return Ok(JsonOp::Sync(json!(false)));
  }

  match resource.unwrap() {
    StreamResource::Stdin(_) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stdin))))
    }
    StreamResource::Stdout(_) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stdout))))
    }
    StreamResource::Stderr(_) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stderr))))
    }
    StreamResource::FsFile(f) => {
      let tokio_file = futures::executor::block_on(f.try_clone())?;
      let std_file = futures::executor::block_on(tokio_file.into_std());
      #[cfg(windows)]
      {
        use winapi::um::consoleapi;

        let handle = get_windows_handle(&std_file)?;
        let mut test_mode: DWORD = 0;
        // If I cannot get mode out of console, it is not a console.
        let result =
          unsafe { consoleapi::GetConsoleMode(handle, &mut test_mode) != 0 };
        Ok(JsonOp::Sync(json!(result)))
      }
      #[cfg(unix)]
      {
        use std::os::unix::io::AsRawFd;
        let raw_fd = std_file.as_raw_fd();
        let result = unsafe { libc::isatty(raw_fd as libc::c_int) == 1 };
        Ok(JsonOp::Sync(json!(result)))
      }
    }
    _ => Ok(JsonOp::Sync(json!(false))),
  }
}
