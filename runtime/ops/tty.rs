// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::HashMap;
use std::io::Error;
#[cfg(windows)]
use std::sync::Arc;

use deno_core::OpState;
#[cfg(unix)]
use deno_core::ResourceId;
use deno_core::op2;
#[cfg(windows)]
use deno_core::parking_lot::Mutex;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_error::builtin_classes::GENERIC_ERROR;
#[cfg(windows)]
use deno_io::WinTtyState;
#[cfg(unix)]
use nix::sys::termios;
use rustyline::Cmd;
use rustyline::Editor;
use rustyline::KeyCode;
use rustyline::KeyEvent;
use rustyline::Modifiers;
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;

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

#[cfg(unix)]
use deno_process::JsNixError;
#[cfg(windows)]
use windows_sys::Win32::System::Console as wincon;

deno_core::extension!(
  deno_tty,
  ops = [op_set_raw, op_console_size, op_read_line_prompt],
  state = |state| {
    #[cfg(unix)]
    state.put(TtyModeStore::default());
    #[cfg(not(unix))]
    let _ = state;
  },
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TtyError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(
    #[from]
    #[inherit]
    deno_core::error::ResourceError,
  ),
  #[class(inherit)]
  #[error("{0}")]
  Io(
    #[from]
    #[inherit]
    Error,
  ),
  #[cfg(unix)]
  #[class(inherit)]
  #[error(transparent)]
  Nix(#[inherit] JsNixError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[inherit] JsErrorBox),
}

// ref: <https://learn.microsoft.com/en-us/windows/console/setconsolemode>
#[cfg(windows)]
const COOKED_MODE: u32 =
  // enable line-by-line input (returns input only after CR is read)
  wincon::ENABLE_LINE_INPUT
  // enables real-time character echo to console display (requires ENABLE_LINE_INPUT)
  | wincon::ENABLE_ECHO_INPUT
  // system handles CTRL-C (with ENABLE_LINE_INPUT, also handles BS, CR, and LF) and other control keys (when using `ReadFile` or `ReadConsole`)
  | wincon::ENABLE_PROCESSED_INPUT;

#[cfg(windows)]
fn mode_raw_input_on(original_mode: u32) -> u32 {
  original_mode & !COOKED_MODE | wincon::ENABLE_VIRTUAL_TERMINAL_INPUT
}

#[cfg(windows)]
fn mode_raw_input_off(original_mode: u32) -> u32 {
  original_mode & !wincon::ENABLE_VIRTUAL_TERMINAL_INPUT | COOKED_MODE
}

#[op2(fast)]
fn op_set_raw(
  state: &mut OpState,
  rid: u32,
  is_raw: bool,
  cbreak: bool,
) -> Result<(), TtyError> {
  let handle_or_fd = state.resource_table.get_fd(rid)?;

  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/windows.rs
  // and https://github.com/kkawakam/rustyline/blob/master/src/tty/unix.rs
  // and https://github.com/crossterm-rs/crossterm/blob/e35d4d2c1cc4c919e36d242e014af75f6127ab50/src/terminal/sys/windows.rs
  // Copyright (c) 2015 Katsu Kawakami & Rustyline authors. MIT license.
  // Copyright (c) 2019 Timon. MIT license.
  #[cfg(windows)]
  {
    use deno_error::JsErrorBox;
    use windows_sys::Win32::Foundation::FALSE;

    let handle = handle_or_fd;

    if cbreak {
      return Err(TtyError::Other(JsErrorBox::not_supported()));
    }

    let mut original_mode: u32 = 0;
    // SAFETY: Win32 call
    if unsafe { wincon::GetConsoleMode(handle, &mut original_mode) } == FALSE {
      return Err(TtyError::Io(Error::last_os_error()));
    }

    let new_mode = if is_raw {
      mode_raw_input_on(original_mode)
    } else {
      mode_raw_input_off(original_mode)
    };

    let stdin_state = state.borrow::<Arc<Mutex<WinTtyState>>>();
    let mut stdin_state = stdin_state.lock();

    if stdin_state.reading {
      let cvar = stdin_state.cvar.clone();

      /* Trick to unblock an ongoing line-buffered read operation if not already pending.
      See https://github.com/libuv/libuv/pull/866 for prior art */
      if original_mode & COOKED_MODE != 0 && !stdin_state.cancelled {
        // SAFETY: Write enter key event to force the console wait to return.
        let record = unsafe {
          use windows_sys::Win32::UI::Input::KeyboardAndMouse::MAPVK_VK_TO_VSC;
          use windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW;
          use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;

          let mut record: wincon::INPUT_RECORD = std::mem::zeroed();
          record.EventType = wincon::KEY_EVENT as u16;
          record.Event.KeyEvent.wVirtualKeyCode = VK_RETURN;
          record.Event.KeyEvent.bKeyDown = 1;
          record.Event.KeyEvent.wRepeatCount = 1;
          record.Event.KeyEvent.uChar.UnicodeChar = '\r' as u16;
          record.Event.KeyEvent.dwControlKeyState = 0;
          record.Event.KeyEvent.wVirtualScanCode =
            MapVirtualKeyW(VK_RETURN as u32, MAPVK_VK_TO_VSC) as u16;
          record
        };
        stdin_state.cancelled = true;

        // SAFETY: Win32 call to open conout$ and save screen state.
        let active_screen_buffer = unsafe {
          use windows_sys::Win32::Foundation::GENERIC_READ;
          use windows_sys::Win32::Foundation::GENERIC_WRITE;
          use windows_sys::Win32::Storage::FileSystem::CreateFileW;
          use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
          use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
          use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;

          /* Save screen state before sending the VK_RETURN event */
          let handle = CreateFileW(
            "conout$"
              .encode_utf16()
              .chain(Some(0))
              .collect::<Vec<_>>()
              .as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
          );

          let mut active_screen_buffer = std::mem::zeroed();
          wincon::GetConsoleScreenBufferInfo(handle, &mut active_screen_buffer);
          windows_sys::Win32::Foundation::CloseHandle(handle);
          active_screen_buffer
        };
        stdin_state.screen_buffer_info = Some(active_screen_buffer);

        // SAFETY: Win32 call to write the VK_RETURN event.
        if unsafe { wincon::WriteConsoleInputW(handle, &record, 1, &mut 0) }
          == FALSE
        {
          return Err(TtyError::Io(Error::last_os_error()));
        }

        /* Wait for read thread to acknowledge the cancellation to ensure that nothing
        interferes with the screen state.
        NOTE: `wait_while` automatically unlocks stdin_state */
        cvar.wait_while(&mut stdin_state, |state: &mut WinTtyState| {
          state.cancelled
        });
      }
    }

    // SAFETY: Win32 call
    if unsafe { wincon::SetConsoleMode(handle, new_mode) } == FALSE {
      return Err(TtyError::Io(Error::last_os_error()));
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

    // SAFETY: Nix crate requires value to implement the AsFd trait
    let raw_fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(handle_or_fd) };

    if is_raw {
      let mut raw = match previous_mode {
        Some(mode) => mode,
        None => {
          // Save original mode.
          let original_mode = termios::tcgetattr(raw_fd)
            .map_err(|e| TtyError::Nix(JsNixError(e)))?;
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
      termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &raw)
        .map_err(|e| TtyError::Nix(JsNixError(e)))?;
    } else {
      // Try restore saved mode.
      if let Some(mode) = tty_mode_store.take(rid) {
        termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &mode)
          .map_err(|e| TtyError::Nix(JsNixError(e)))?;
      }
    }

    Ok(())
  }
}

#[op2(fast)]
fn op_console_size(
  state: &mut OpState,
  #[buffer] result: &mut [u32],
) -> Result<(), TtyError> {
  fn check_console_size(
    state: &mut OpState,
    result: &mut [u32],
    rid: u32,
  ) -> Result<(), TtyError> {
    let fd = state.resource_table.get_fd(rid)?;
    let size = console_size_from_fd(fd)?;
    result[0] = size.cols;
    result[1] = size.rows;
    Ok(())
  }

  // Since stdio might be piped we try to get the size of the console for all
  // of them and return the first one that succeeds.
  for rid in [0, 1, 2] {
    if check_console_size(state, result, rid).is_ok() {
      return Ok(());
    }
  }

  Err(TtyError::Other(JsErrorBox::generic(
    "Could not get console size: stdin, stdout, and stderr are not connected to a terminal",
  )))
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

/// Get the console size from stderr (fd 2) directly, without needing
/// a StdFile handle.
pub fn console_size_of_stderr() -> Result<ConsoleSize, std::io::Error> {
  #[cfg(windows)]
  {
    // SAFETY: GetStdHandle with STD_ERROR_HANDLE always returns a valid handle.
    let handle = unsafe { wincon::GetStdHandle(wincon::STD_ERROR_HANDLE) };
    console_size_from_fd(handle)
  }
  #[cfg(unix)]
  {
    console_size_from_fd(2)
  }
}

#[cfg(windows)]
fn console_size_from_fd(
  handle: std::os::windows::io::RawHandle,
) -> Result<ConsoleSize, std::io::Error> {
  // SAFETY: Win32 calls
  unsafe {
    let mut bufinfo: wincon::CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();

    if wincon::GetConsoleScreenBufferInfo(handle, &mut bufinfo) == 0 {
      return Err(Error::last_os_error());
    }

    // calculate the size of the visible window
    // * use over/under-flow protections b/c MSDN docs only imply that srWindow components are all non-negative
    // * ref: <https://docs.microsoft.com/en-us/windows/console/console-screen-buffer-info-str> @@ <https://archive.is/sfjnm>
    let cols = std::cmp::max(
      bufinfo.srWindow.Right as i32 - bufinfo.srWindow.Left as i32 + 1,
      0,
    ) as u32;
    let rows = std::cmp::max(
      bufinfo.srWindow.Bottom as i32 - bufinfo.srWindow.Top as i32 + 1,
      0,
    ) as u32;

    Ok(ConsoleSize { cols, rows })
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

deno_error::js_error_wrapper!(ReadlineError, JsReadlineError, |err| {
  match err {
    ReadlineError::Io(e) => e.get_class(),
    ReadlineError::Eof => GENERIC_ERROR.into(),
    ReadlineError::Interrupted => GENERIC_ERROR.into(),
    #[cfg(unix)]
    ReadlineError::Errno(e) => JsNixError(*e).get_class(),
    _ => GENERIC_ERROR.into(),
  }
});

#[op2]
#[string]
pub fn op_read_line_prompt(
  #[string] prompt_text: &str,
  #[string] default_value: &str,
) -> Result<Option<String>, JsReadlineError> {
  let _terminal_input_guard = deno_permissions::prompter::lock_terminal_input();
  let mut editor = Editor::<(), rustyline::history::DefaultHistory>::new()
    .expect("Failed to create editor.");

  editor.set_keyseq_timeout(Some(1));
  editor
    .bind_sequence(KeyEvent(KeyCode::Esc, Modifiers::empty()), Cmd::Interrupt);

  let read_result =
    editor.readline_with_initial(prompt_text, (default_value, ""));
  match read_result {
    Ok(line) => Ok(Some(line)),
    Err(ReadlineError::Interrupted) => {
      // SAFETY: Disable raw mode and raise SIGINT.
      unsafe {
        libc::raise(libc::SIGINT);
      }
      Ok(None)
    }
    Err(ReadlineError::Eof) => Ok(None),
    Err(err) => Err(JsReadlineError(err)),
  }
}
