// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::io::std_file_resource;
use super::io::StreamResource;
use super::io::StreamResourceHolder;
use deno_core::error::bad_resource_id;
use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;
use std::io::Error;

#[cfg(unix)]
use nix::sys::termios;

#[cfg(windows)]
use deno_core::error::custom_error;
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
) -> Result<std::os::windows::io::RawHandle, AnyError> {
  use std::os::windows::io::AsRawHandle;
  use winapi::um::handleapi;

  let handle = f.as_raw_handle();
  if handle == handleapi::INVALID_HANDLE_VALUE {
    return Err(Error::last_os_error());
  } else if handle.is_null() {
    return Err(custom_error("ReferenceError", "null handle"));
  }
  Ok(handle)
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_set_raw", op_set_raw);
  super::reg_json_sync(rt, "op_isatty", op_isatty);
  super::reg_json_sync(rt, "op_console_size", op_console_size);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetRawOptions {
  cbreak: bool,
}

#[derive(Deserialize)]
struct SetRawArgs {
  rid: u32,
  mode: bool,
  options: SetRawOptions,
}

fn op_set_raw(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.setRaw");

  let args: SetRawArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let is_raw = args.mode;
  let cbreak = args.options.cbreak;

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

    let resource_holder =
      state.resource_table.get_mut::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(bad_resource_id());
    }
    if cbreak {
      return Err(not_supported());
    }
    let resource_holder = resource_holder.unwrap();

    // For now, only stdin.
    let handle = match &mut resource_holder.resource {
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
              return Err(resource_unavailable());
            }
          }
        } else {
          return Err(resource_unavailable());
        }
      }
      _ => {
        return Err(bad_resource_id());
      }
    };

    if handle == handleapi::INVALID_HANDLE_VALUE {
      return Err(Error::last_os_error());
    } else if handle.is_null() {
      return Err(custom_error("ReferenceError", "null handle"));
    }
    let mut original_mode: DWORD = 0;
    if unsafe { consoleapi::GetConsoleMode(handle, &mut original_mode) }
      == FALSE
    {
      return Err(Error::last_os_error());
    }
    let new_mode = if is_raw {
      original_mode & !RAW_MODE_MASK
    } else {
      original_mode | RAW_MODE_MASK
    };
    if unsafe { consoleapi::SetConsoleMode(handle, new_mode) } == FALSE {
      return Err(Error::last_os_error());
    }

    Ok(json!({}))
  }
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;

    let resource_holder =
      state.resource_table.get_mut::<StreamResourceHolder>(rid);
    if resource_holder.is_none() {
      return Err(bad_resource_id());
    }

    if is_raw {
      let (raw_fd, maybe_tty_mode) =
        match &mut resource_holder.unwrap().resource {
          StreamResource::FsFile(Some((f, ref mut metadata))) => {
            (f.as_raw_fd(), &mut metadata.tty.mode)
          }
          StreamResource::FsFile(None) => return Err(resource_unavailable()),
          _ => {
            return Err(not_supported());
          }
        };

      if maybe_tty_mode.is_none() {
        // Save original mode.
        let original_mode = termios::tcgetattr(raw_fd)?;
        maybe_tty_mode.replace(original_mode);
      }

      let mut raw = maybe_tty_mode.clone().unwrap();

      raw.input_flags &= !(termios::InputFlags::BRKINT
        | termios::InputFlags::ICRNL
        | termios::InputFlags::INPCK
        | termios::InputFlags::ISTRIP
        | termios::InputFlags::IXON);

      raw.control_flags |= termios::ControlFlags::CS8;

      raw.local_flags &= !(termios::LocalFlags::ECHO
        | termios::LocalFlags::ICANON
        | termios::LocalFlags::IEXTEN);
      if !cbreak {
        raw.local_flags &= !(termios::LocalFlags::ISIG);
      }
      raw.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 1;
      raw.control_chars[termios::SpecialCharacterIndices::VTIME as usize] = 0;
      termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &raw)?;
      Ok(json!({}))
    } else {
      // Try restore saved mode.
      let (raw_fd, maybe_tty_mode) =
        match &mut resource_holder.unwrap().resource {
          StreamResource::FsFile(Some((f, ref mut metadata))) => {
            (f.as_raw_fd(), &mut metadata.tty.mode)
          }
          StreamResource::FsFile(None) => {
            return Err(resource_unavailable());
          }
          _ => {
            return Err(bad_resource_id());
          }
        };

      if let Some(mode) = maybe_tty_mode.take() {
        termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &mode)?;
      }

      Ok(json!({}))
    }
  }
}

#[derive(Deserialize)]
struct IsattyArgs {
  rid: u32,
}

fn op_isatty(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: IsattyArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let isatty: bool = std_file_resource(state, rid as u32, move |r| match r {
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
    _ => Ok(false),
  })?;
  Ok(json!(isatty))
}

#[derive(Deserialize)]
struct ConsoleSizeArgs {
  rid: u32,
}

#[derive(Serialize)]
struct ConsoleSize {
  columns: u32,
  rows: u32,
}

fn op_console_size(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.consoleSize");

  let args: ConsoleSizeArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let size = std_file_resource(state, rid as u32, move |r| match r {
    Ok(std_file) => {
      #[cfg(windows)]
      {
        use std::os::windows::io::AsRawHandle;
        let handle = std_file.as_raw_handle();

        unsafe {
          let mut bufinfo: winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO =
            std::mem::zeroed();

          if winapi::um::wincon::GetConsoleScreenBufferInfo(
            handle,
            &mut bufinfo,
          ) == 0
          {
            return Err(Error::last_os_error());
          }

          Ok(ConsoleSize {
            columns: bufinfo.dwSize.X as u32,
            rows: bufinfo.dwSize.Y as u32,
          })
        }
      }

      #[cfg(unix)]
      {
        use std::os::unix::io::AsRawFd;

        let fd = std_file.as_raw_fd();
        unsafe {
          let mut size: libc::winsize = std::mem::zeroed();
          if libc::ioctl(fd, libc::TIOCGWINSZ, &mut size as *mut _) != 0 {
            return Err(Error::last_os_error().into());
          }

          // TODO (caspervonb) return a tuple instead
          Ok(ConsoleSize {
            columns: size.ws_col as u32,
            rows: size.ws_row as u32,
          })
        }
      }
    }
    Err(_) => Err(bad_resource_id()),
  })?;

  Ok(json!(size))
}
