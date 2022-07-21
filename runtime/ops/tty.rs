// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::io::StdFileResource;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ResourceId;
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
    return Err(Error::last_os_error().into());
  } else if handle.is_null() {
    return Err(custom_error("ReferenceError", "null handle"));
  }
  Ok(handle)
}

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_set_raw::decl(),
      op_isatty::decl(),
      op_console_size::decl(),
    ])
    .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRawOptions {
  cbreak: bool,
}

#[derive(Deserialize)]
pub struct SetRawArgs {
  rid: ResourceId,
  mode: bool,
  options: SetRawOptions,
}

#[op]
fn op_set_raw(state: &mut OpState, args: SetRawArgs) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.setRaw");

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

    if cbreak {
      return Err(deno_core::error::not_supported());
    }

    StdFileResource::with_file(state, rid, move |std_file| {
      let handle = std_file.as_raw_handle();

      if handle == handleapi::INVALID_HANDLE_VALUE {
        return Err(Error::last_os_error().into());
      } else if handle.is_null() {
        return Err(custom_error("ReferenceError", "null handle"));
      }
      let mut original_mode: DWORD = 0;
      // SAFETY: winapi call
      if unsafe { consoleapi::GetConsoleMode(handle, &mut original_mode) }
        == FALSE
      {
        return Err(Error::last_os_error().into());
      }
      let new_mode = if is_raw {
        original_mode & !RAW_MODE_MASK
      } else {
        original_mode | RAW_MODE_MASK
      };
      // SAFETY: winapi call
      if unsafe { consoleapi::SetConsoleMode(handle, new_mode) } == FALSE {
        return Err(Error::last_os_error().into());
      }

      Ok(())
    })
  }
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;

    StdFileResource::with_file_and_metadata(
      state,
      rid,
      move |std_file, meta_data| {
        let raw_fd = std_file.as_raw_fd();
        let maybe_tty_mode = &mut meta_data.tty.mode;

        if is_raw {
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
          raw.control_chars[termios::SpecialCharacterIndices::VMIN as usize] =
            1;
          raw.control_chars[termios::SpecialCharacterIndices::VTIME as usize] =
            0;
          termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &raw)?;
        } else {
          // Try restore saved mode.
          if let Some(mode) = maybe_tty_mode.take() {
            termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &mode)?;
          }
        }

        Ok(())
      },
    )
  }
}

#[op]
fn op_isatty(state: &mut OpState, rid: ResourceId) -> Result<bool, AnyError> {
  let isatty: bool = StdFileResource::with_file(state, rid, move |std_file| {
    #[cfg(windows)]
    {
      use winapi::shared::minwindef::FALSE;
      use winapi::um::consoleapi;

      let handle = get_windows_handle(std_file)?;
      let mut test_mode: DWORD = 0;
      // If I cannot get mode out of console, it is not a console.
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      Ok(unsafe { consoleapi::GetConsoleMode(handle, &mut test_mode) != FALSE })
    }
    #[cfg(unix)]
    {
      use std::os::unix::io::AsRawFd;
      let raw_fd = std_file.as_raw_fd();
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      Ok(unsafe { libc::isatty(raw_fd as libc::c_int) == 1 })
    }
  })?;
  Ok(isatty)
}

#[derive(Serialize)]
struct ConsoleSize {
  columns: u32,
  rows: u32,
}

#[op]
fn op_console_size(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<ConsoleSize, AnyError> {
  super::check_unstable(state, "Deno.consoleSize");

  let size = StdFileResource::with_file(state, rid, move |std_file| {
    #[cfg(windows)]
    {
      use std::os::windows::io::AsRawHandle;
      let handle = std_file.as_raw_handle();

      // SAFETY: winapi calls
      unsafe {
        let mut bufinfo: winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO =
          std::mem::zeroed();

        if winapi::um::wincon::GetConsoleScreenBufferInfo(handle, &mut bufinfo)
          == 0
        {
          return Err(Error::last_os_error().into());
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
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
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
  })?;

  Ok(size)
}
