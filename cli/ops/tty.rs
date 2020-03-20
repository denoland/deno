use super::dispatch_json::JsonOp;
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::ops::json_op;
use crate::state::State;
use atty;
use deno_core::*;
#[cfg(unix)]
use nix::sys::termios;
use serde_derive::Deserialize;
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
) -> Result<std::os::windows::io::RawHandle, OpError> {
  use std::os::windows::io::AsRawHandle;
  use winapi::um::handleapi;

  let handle = f.as_raw_handle();
  if handle == handleapi::INVALID_HANDLE_VALUE {
    return Err(OpError::from(std::io::Error::last_os_error()));
  } else if handle.is_null() {
    return Err(OpError::other("null handle".to_owned()));
  }
  Ok(handle)
}

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_set_raw", s.core_op(json_op(s.stateful_op(op_set_raw))));
  i.register_op("op_isatty", s.core_op(json_op(s.stateful_op(op_isatty))));
}

#[derive(Deserialize)]
struct SetRawArgs {
  rid: u32,
  mode: bool,
}

pub fn op_set_raw(
  state_: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SetRawArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let is_raw = args.mode;

  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/windows.rs
  // and https://github.com/kkawakam/rustyline/blob/master/src/tty/unix.rs
  // and https://github.com/crossterm-rs/crossterm/blob/e35d4d2c1cc4c919e36d242e014af75f6127ab50/src/terminal/sys/windows.rs
  // Copyright (c) 2015 Katsu Kawakami & Rustyline authors. MIT license.
  // Copyright (c) 2019 Timon. MIT license.
  #[cfg(windows)]
  {
    use std::os::windows::io::AsRawHandle;
    use winapi::shared::minwindef::FALSE;
    use winapi::um::{consoleapi, handleapi};

    let state = state_.borrow_mut();
    let resource_holder = state.resource_table.get::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(OpError::bad_resource_id());
    }

    // For now, only stdin.
    let handle = match &resource_holder.unwrap().resource {
      StreamResource::Stdin(_, _) => std::io::stdin().as_raw_handle(),
      StreamResource::FsFile(f, _) => {
        let tokio_file = futures::executor::block_on(f.try_clone())?;
        let std_file = futures::executor::block_on(tokio_file.into_std());
        std_file.as_raw_handle()
      }
      _ => {
        return Err(OpError::other("Not supported".to_owned()));
      }
    };

    if handle == handleapi::INVALID_HANDLE_VALUE {
      return Err(OpError::from(std::io::Error::last_os_error()));
    } else if handle.is_null() {
      return Err(OpError::other("null handle".to_owned()));
    }
    let mut original_mode: DWORD = 0;
    if unsafe { consoleapi::GetConsoleMode(handle, &mut original_mode) }
      == FALSE
    {
      return Err(OpError::from(std::io::Error::last_os_error()));
    }
    let new_mode = if is_raw {
      original_mode & !RAW_MODE_MASK
    } else {
      original_mode | RAW_MODE_MASK
    };
    if unsafe { consoleapi::SetConsoleMode(handle, new_mode) } == FALSE {
      return Err(OpError::from(std::io::Error::last_os_error()));
    }

    Ok(JsonOp::Sync(json!({})))
  }
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;

    let mut state = state_.borrow_mut();
    let resource_holder =
      state.resource_table.get_mut::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(OpError::bad_resource_id());
    }

    if is_raw {
      let (raw_fd, maybe_tty_mode) =
        match &mut resource_holder.unwrap().resource {
          StreamResource::Stdin(_, ref mut metadata) => {
            (std::io::stdin().as_raw_fd(), &mut metadata.mode)
          }
          StreamResource::FsFile(f, ref mut metadata) => {
            let tokio_file = futures::executor::block_on(f.try_clone())?;
            let std_file = futures::executor::block_on(tokio_file.into_std());
            (std_file.as_raw_fd(), &mut metadata.tty.mode)
          }
          _ => {
            return Err(OpError::other("Not supported".to_owned()));
          }
        };

      if maybe_tty_mode.is_some() {
        // Already raw. Skip.
        return Ok(JsonOp::Sync(json!({})));
      }

      let original_mode = termios::tcgetattr(raw_fd)?;
      let mut raw = original_mode.clone();
      // Save original mode.
      maybe_tty_mode.replace(original_mode);

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
      Ok(JsonOp::Sync(json!({})))
    } else {
      // Try restore saved mode.
      let (raw_fd, maybe_tty_mode) =
        match &mut resource_holder.unwrap().resource {
          StreamResource::Stdin(_, ref mut metadata) => {
            (std::io::stdin().as_raw_fd(), &mut metadata.mode)
          }
          StreamResource::FsFile(f, ref mut metadata) => {
            let tokio_file = futures::executor::block_on(f.try_clone())?;
            let std_file = futures::executor::block_on(tokio_file.into_std());
            (std_file.as_raw_fd(), &mut metadata.tty.mode)
          }
          _ => {
            return Err(OpError::other("Not supported".to_owned()));
          }
        };

      if let Some(mode) = maybe_tty_mode.take() {
        termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &mode)?;
      }

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
) -> Result<JsonOp, OpError> {
  let args: IsattyArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let state = state_.borrow_mut();
  if !state.resource_table.has(rid) {
    return Err(OpError::bad_resource_id());
  }

  let resource_holder = state.resource_table.get::<StreamResourceHolder>(rid);
  if resource_holder.is_none() {
    return Ok(JsonOp::Sync(json!(false)));
  }

  match &resource_holder.unwrap().resource {
    StreamResource::Stdin(_, _) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stdin))))
    }
    StreamResource::Stdout(_) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stdout))))
    }
    StreamResource::Stderr(_) => {
      Ok(JsonOp::Sync(json!(atty::is(atty::Stream::Stderr))))
    }
    StreamResource::FsFile(f, _) => {
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
