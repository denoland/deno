// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::Error;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;

#[cfg(unix)]
use deno_core::ResourceId;
#[cfg(unix)]
use nix::sys::termios;
#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::HashMap;

#[cfg(unix)]
#[derive(Default, Clone)]
struct TtyModeStore(
  std::rc::Rc<RefCell<HashMap<ResourceId, termios::Termios>>>,
);

#[cfg(unix)]
impl TtyModeStore {
  pub fn get(&self, id: ResourceId) -> Option<termios::Termios> {
    self.0.borrow().get(&id).map(ToOwned::to_owned)
  }

  pub fn take(&self, id: ResourceId) -> Option<termios::Termios> {
    self.0.borrow_mut().remove(&id)
  }

  pub fn set(&self, id: ResourceId, mode: termios::Termios) {
    self.0.borrow_mut().insert(id, mode);
  }
}

#[cfg(windows)]
use winapi::shared::minwindef::DWORD;
#[cfg(windows)]
use winapi::um::wincon;

deno_core::extension!(
  deno_tty,
  ops = [op_stdin_set_raw, op_isatty, op_console_size],
  state = |state| {
    #[cfg(unix)]
    state.put(TtyModeStore::default());
  },
);

// ref: <https://learn.microsoft.com/en-us/windows/console/setconsolemode>
#[cfg(windows)]
const COOKED_MODE: DWORD =
  // enable line-by-line input (returns input only after CR is read)
  wincon::ENABLE_LINE_INPUT
  // enables real-time character echo to console display (requires ENABLE_LINE_INPUT)
  | wincon::ENABLE_ECHO_INPUT
  // system handles CTRL-C (with ENABLE_LINE_INPUT, also handles BS, CR, and LF) and other control keys (when using `ReadFile` or `ReadConsole`)
  | wincon::ENABLE_PROCESSED_INPUT;

#[cfg(windows)]
fn mode_raw_input_on(original_mode: DWORD) -> DWORD {
  original_mode & !COOKED_MODE | wincon::ENABLE_VIRTUAL_TERMINAL_INPUT
}

#[cfg(windows)]
fn mode_raw_input_off(original_mode: DWORD) -> DWORD {
  original_mode & !wincon::ENABLE_VIRTUAL_TERMINAL_INPUT | COOKED_MODE
}

#[op2(fast)]
fn op_stdin_set_raw(
  state: &mut OpState,
  is_raw: bool,
  cbreak: bool,
) -> Result<(), AnyError> {
  let rid = 0; // stdin is always rid=0
  let handle_or_fd = state.resource_table.get_fd(rid)?;

  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/windows.rs
  // and https://github.com/kkawakam/rustyline/blob/master/src/tty/unix.rs
  // and https://github.com/crossterm-rs/crossterm/blob/e35d4d2c1cc4c919e36d242e014af75f6127ab50/src/terminal/sys/windows.rs
  // Copyright (c) 2015 Katsu Kawakami & Rustyline authors. MIT license.
  // Copyright (c) 2019 Timon. MIT license.
  #[cfg(windows)]
  {
    use winapi::shared::minwindef::FALSE;
    use winapi::um::consoleapi;

    let handle = handle_or_fd;

    if cbreak {
      return Err(deno_core::error::not_supported());
    }

    let mut original_mode: DWORD = 0;
    // SAFETY: winapi call
    if unsafe { consoleapi::GetConsoleMode(handle, &mut original_mode) }
      == FALSE
    {
      return Err(Error::last_os_error().into());
    }

    let new_mode = if is_raw {
      mode_raw_input_on(original_mode)
    } else {
      mode_raw_input_off(original_mode)
    };

    // SAFETY: winapi call
    if unsafe { consoleapi::SetConsoleMode(handle, new_mode) } == FALSE {
      return Err(Error::last_os_error().into());
    }

    Ok(())
  }
  #[cfg(unix)]
  {
    fn prepare_stdio() {
      // SAFETY: Save current state of stdio and restore it when we exit.
      unsafe {
        use libc::atexit;
        use libc::tcgetattr;
        use libc::tcsetattr;
        use libc::termios;
        use once_cell::sync::OnceCell;

        // Only save original state once.
        static ORIG_TERMIOS: OnceCell<Option<termios>> = OnceCell::new();
        ORIG_TERMIOS.get_or_init(|| {
          let mut termios = std::mem::zeroed::<termios>();
          if tcgetattr(libc::STDIN_FILENO, &mut termios) == 0 {
            extern "C" fn reset_stdio() {
              // SAFETY: Reset the stdio state.
              unsafe {
                tcsetattr(
                  libc::STDIN_FILENO,
                  0,
                  &ORIG_TERMIOS.get().unwrap().unwrap(),
                )
              };
            }

            atexit(reset_stdio);
            return Some(termios);
          }

          None
        });
      }
    }

    prepare_stdio();
    let tty_mode_store = state.borrow::<TtyModeStore>().clone();
    let previous_mode = tty_mode_store.get(rid);

    let raw_fd = handle_or_fd;

    if is_raw {
      let mut raw = match previous_mode {
        Some(mode) => mode,
        None => {
          // Save original mode.
          let original_mode = termios::tcgetattr(raw_fd)?;
          tty_mode_store.set(rid, original_mode.clone());
          original_mode
        }
      };

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
    } else {
      // Try restore saved mode.
      if let Some(mode) = tty_mode_store.take(rid) {
        termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &mode)?;
      }
    }

    Ok(())
  }
}

#[op2(fast)]
fn op_isatty(state: &mut OpState, rid: u32) -> Result<bool, AnyError> {
  let handle = state.resource_table.get_handle(rid)?;
  Ok(handle.is_terminal())
}

#[op2(fast)]
fn op_console_size(
  state: &mut OpState,
  #[buffer] result: &mut [u32],
) -> Result<(), AnyError> {
  fn check_console_size(
    state: &mut OpState,
    result: &mut [u32],
    rid: u32,
  ) -> Result<(), AnyError> {
    let fd = state.resource_table.get_fd(rid)?;
    let size = console_size_from_fd(fd)?;
    result[0] = size.cols;
    result[1] = size.rows;
    Ok(())
  }

  let mut last_result = Ok(());
  // Since stdio might be piped we try to get the size of the console for all
  // of them and return the first one that succeeds.
  for rid in [0, 1, 2] {
    last_result = check_console_size(state, result, rid);
    if last_result.is_ok() {
      return last_result;
    }
  }

  last_result
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ConsoleSize {
  pub cols: u32,
  pub rows: u32,
}

pub fn console_size(
  std_file: &std::fs::File,
) -> Result<ConsoleSize, std::io::Error> {
  #[cfg(windows)]
  {
    use std::os::windows::io::AsRawHandle;
    let handle = std_file.as_raw_handle();
    console_size_from_fd(handle)
  }
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    let fd = std_file.as_raw_fd();
    console_size_from_fd(fd)
  }
}

#[cfg(windows)]
fn console_size_from_fd(
  handle: std::os::windows::io::RawHandle,
) -> Result<ConsoleSize, std::io::Error> {
  // SAFETY: winapi calls
  unsafe {
    let mut bufinfo: winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO =
      std::mem::zeroed();

    if winapi::um::wincon::GetConsoleScreenBufferInfo(handle, &mut bufinfo) == 0
    {
      return Err(Error::last_os_error());
    }
    Ok(ConsoleSize {
      cols: bufinfo.dwSize.X as u32,
      rows: bufinfo.dwSize.Y as u32,
    })
  }
}

#[cfg(not(windows))]
fn console_size_from_fd(
  fd: std::os::unix::prelude::RawFd,
) -> Result<ConsoleSize, std::io::Error> {
  // SAFETY: libc calls
  unsafe {
    let mut size: libc::winsize = std::mem::zeroed();
    if libc::ioctl(fd, libc::TIOCGWINSZ, &mut size as *mut _) != 0 {
      return Err(Error::last_os_error());
    }
    Ok(ConsoleSize {
      cols: size.ws_col as u32,
      rows: size.ws_row as u32,
    })
  }
}

#[cfg(all(test, windows))]
mod tests {
  #[test]
  fn test_winos_raw_mode_transitions() {
    use crate::ops::tty::mode_raw_input_off;
    use crate::ops::tty::mode_raw_input_on;

    let known_off_modes =
      [0xf7 /* Win10/CMD */, 0x1f7 /* Win10/WinTerm */];
    let known_on_modes =
      [0x2f0 /* Win10/CMD */, 0x3f0 /* Win10/WinTerm */];

    // assert known transitions
    assert_eq!(known_on_modes[0], mode_raw_input_on(known_off_modes[0]));
    assert_eq!(known_on_modes[1], mode_raw_input_on(known_off_modes[1]));

    // assert ON-OFF round-trip is neutral
    assert_eq!(
      known_off_modes[0],
      mode_raw_input_off(mode_raw_input_on(known_off_modes[0]))
    );
    assert_eq!(
      known_off_modes[1],
      mode_raw_input_off(mode_raw_input_on(known_off_modes[1]))
    );
  }
}
