use super::dispatch_json::JsonOp;
use super::io::std_file_resource;
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;
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

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_set_raw", s.stateful_json_op2(op_set_raw));
  i.register_op("op_isatty", s.stateful_json_op2(op_isatty));
}

#[derive(Deserialize)]
struct SetRawArgs {
  rid: u32,
  mode: bool,
}

pub fn op_set_raw(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.setRaw");
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

    let mut resource_table = isolate_state.resource_table.borrow_mut();
    let resource_holder = resource_table.get_mut::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(OpError::bad_resource_id());
    }
    let resource_holder = resource_holder.unwrap();

    // For now, only stdin.
    let handle = match &mut resource_holder.resource {
      StreamResource::Stdin(_, _) => std::io::stdin().as_raw_handle(),
      StreamResource::FsFile(ref mut option_file_metadata) => {
        if let Some((tokio_file, metadata)) = option_file_metadata.take() {
          match tokio_file.try_into_std() {
            Ok(std_file) => {
              let raw_handle = std_file.as_raw_handle();
              // Turn the std_file handle back into a tokio file, put it back
              // in the resource table.
              let tokio_file = tokio::fs::File::from_std(std_file);
              resource_holder.resource =
                StreamResource::FsFile(Some((tokio_file, metadata)));
              // return the result.
              raw_handle
            }
            Err(tokio_file) => {
              // This function will return an error containing the file if
              // some operation is in-flight.
              resource_holder.resource =
                StreamResource::FsFile(Some((tokio_file, metadata)));
              return Err(OpError::resource_unavailable());
            }
          }
        } else {
          return Err(OpError::resource_unavailable());
        }
      }
      _ => {
        return Err(OpError::bad_resource_id());
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

    let mut resource_table = isolate_state.resource_table.borrow_mut();
    let resource_holder = resource_table.get_mut::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(OpError::bad_resource_id());
    }

    if is_raw {
      let (raw_fd, maybe_tty_mode) =
        match &mut resource_holder.unwrap().resource {
          StreamResource::Stdin(_, ref mut metadata) => {
            (std::io::stdin().as_raw_fd(), &mut metadata.mode)
          }
          StreamResource::FsFile(Some((f, ref mut metadata))) => {
            (f.as_raw_fd(), &mut metadata.tty.mode)
          }
          StreamResource::FsFile(None) => {
            return Err(OpError::resource_unavailable())
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
          StreamResource::FsFile(Some((f, ref mut metadata))) => {
            (f.as_raw_fd(), &mut metadata.tty.mode)
          }
          StreamResource::FsFile(None) => {
            return Err(OpError::resource_unavailable());
          }
          _ => {
            return Err(OpError::bad_resource_id());
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
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: IsattyArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let isatty: bool =
    std_file_resource(&mut resource_table, rid as u32, move |r| match r {
      Ok(std_file) => {
        #[cfg(windows)]
        {
          use winapi::um::consoleapi;

          let handle = get_windows_handle(&std_file)?;
          let mut test_mode: DWORD = 0;
          // If I cannot get mode out of console, it is not a console.
          Ok(unsafe { consoleapi::GetConsoleMode(handle, &mut test_mode) != 0 })
        }
        #[cfg(unix)]
        {
          use std::os::unix::io::AsRawFd;
          let raw_fd = std_file.as_raw_fd();
          Ok(unsafe { libc::isatty(raw_fd as libc::c_int) == 1 })
        }
      }
      Err(StreamResource::FsFile(_)) => unreachable!(),
      Err(StreamResource::Stdin(_, _)) => Ok(atty::is(atty::Stream::Stdin)),
      _ => Ok(false),
    })?;
  Ok(JsonOp::Sync(json!(isatty)))
}
