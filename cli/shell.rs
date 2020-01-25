// This file was forked from Cargo on 2019.05.29:
// https://github.com/rust-lang/cargo/blob/edd874/src/cargo/core/shell.rs
// Cargo is MIT licenced:
// https://github.com/rust-lang/cargo/blob/edd874/LICENSE-MIT

use std::fmt;
use std::io::prelude::*;

use atty;
use deno_core::ErrBox;
use termcolor::Color::Green;
use termcolor::{self, Color, ColorSpec, StandardStream, WriteColor};

/// The requested verbosity of output.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Verbosity {
  Verbose,
  Normal,
  Quiet,
}

/// An abstraction around a `Write`able object that remembers preferences for output verbosity and
/// color.
pub struct Shell {
  /// the `Write`able object, either with or without color support (represented by different enum
  /// variants)
  err: ShellOut,
  /// How verbose messages should be
  verbosity: Verbosity,
  /// Flag that indicates the current line needs to be cleared before
  /// printing. Used when a progress bar is currently displayed.
  needs_clear: bool,
}

impl fmt::Debug for Shell {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.err {
      /*
      ShellOut::Write(_) => f
        .debug_struct("Shell")
        .field("verbosity", &self.verbosity)
        .finish(),
      */
      ShellOut::Stream { color_choice, .. } => f
        .debug_struct("Shell")
        .field("verbosity", &self.verbosity)
        .field("color_choice", &color_choice)
        .finish(),
    }
  }
}

/// A `Write`able object, either with or without color support
enum ShellOut {
  /// A plain write object without color support
  // TODO(ry) Disabling this type of output because it makes Shell
  // not thread safe and thus not includable in ThreadSafeState.
  // But I think we will want this in the future.
  //Write(Box<dyn Write>),
  /// Color-enabled stdio, with information on whether color should be used
  Stream {
    stream: StandardStream,
    tty: bool,
    color_choice: ColorChoice,
  },
}

/// Whether messages should use color output
#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
pub enum ColorChoice {
  /// Force color output
  Always,
  /// Force disable color output
  Never,
  /// Intelligently guess whether to use color output
  CargoAuto,
}

impl Shell {
  /// Creates a new shell (color choice and verbosity), defaulting to 'auto' color and verbose
  /// output.
  pub fn new() -> Shell {
    Shell {
      err: ShellOut::Stream {
        stream: StandardStream::stderr(
          ColorChoice::CargoAuto.to_termcolor_color_choice(),
        ),
        color_choice: ColorChoice::CargoAuto,
        tty: atty::is(atty::Stream::Stderr),
      },
      verbosity: Verbosity::Verbose,
      needs_clear: false,
    }
  }

  /// Prints a message, where the status will have `color` color, and can be justified. The
  /// messages follows without color.
  fn print(
    &mut self,
    status: &dyn fmt::Display,
    message: Option<&dyn fmt::Display>,
    color: Color,
    justified: bool,
  ) -> Result<(), ErrBox> {
    match self.verbosity {
      Verbosity::Quiet => Ok(()),
      _ => {
        if self.needs_clear {
          self.err_erase_line();
        }
        self.err.print(status, message, color, justified)
      }
    }
  }

  /// Erase from cursor to end of line.
  pub fn err_erase_line(&mut self) {
    if let ShellOut::Stream { tty: true, .. } = self.err {
      imp::err_erase_line(self);
      self.needs_clear = false;
    }
  }

  /// Shortcut to right-align and color green a status message.
  pub fn status<T, U>(&mut self, status: T, message: U) -> Result<(), ErrBox>
  where
    T: fmt::Display,
    U: fmt::Display,
  {
    self.print(&status, Some(&message), Green, false)
  }
}

impl Default for Shell {
  fn default() -> Self {
    Self::new()
  }
}

impl ShellOut {
  /// Prints out a message with a status. The status comes first, and is bold plus the given
  /// color. The status can be justified, in which case the max width that will right align is
  /// 12 chars.
  fn print(
    &mut self,
    status: &dyn fmt::Display,
    message: Option<&dyn fmt::Display>,
    color: Color,
    justified: bool,
  ) -> Result<(), ErrBox> {
    match *self {
      ShellOut::Stream { ref mut stream, .. } => {
        stream.reset()?;
        stream
          .set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
        if justified {
          write!(stream, "{:>12}", status)?;
        } else {
          write!(stream, "{}", status)?;
        }
        stream.reset()?;
        match message {
          Some(message) => writeln!(stream, " {}", message)?,
          None => write!(stream, " ")?,
        }
      }
    }
    Ok(())
  }

  /// Gets this object as a `io::Write`.
  fn as_write(&mut self) -> &mut dyn Write {
    match *self {
      ShellOut::Stream { ref mut stream, .. } => stream,
      // ShellOut::Write(ref mut w) => w,
    }
  }
}

impl ColorChoice {
  /// Converts our color choice to termcolor's version.
  fn to_termcolor_color_choice(self) -> termcolor::ColorChoice {
    match self {
      ColorChoice::Always => termcolor::ColorChoice::Always,
      ColorChoice::Never => termcolor::ColorChoice::Never,
      ColorChoice::CargoAuto => {
        if atty::is(atty::Stream::Stderr) {
          termcolor::ColorChoice::Auto
        } else {
          termcolor::ColorChoice::Never
        }
      }
    }
  }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod imp {
  use super::Shell;

  pub fn err_erase_line(shell: &mut Shell) {
    // This is the "EL - Erase in Line" sequence. It clears from the cursor
    // to the end of line.
    // https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_sequences
    let _ = shell.err.as_write().write_all(b"\x1B[K");
  }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
mod imp {
  pub(super) use super::default_err_erase_line as err_erase_line;

  pub fn stderr_width() -> Option<usize> {
    None
  }
}

#[cfg(windows)]
mod imp {
  use std::{cmp, mem, ptr};
  use winapi::um::fileapi::*;
  use winapi::um::handleapi::*;
  use winapi::um::processenv::*;
  use winapi::um::winbase::*;
  use winapi::um::wincon::*;
  use winapi::um::winnt::*;

  pub(super) use super::default_err_erase_line as err_erase_line;

  pub fn stderr_width() -> Option<usize> {
    unsafe {
      let stdout = GetStdHandle(STD_ERROR_HANDLE);
      let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
      if GetConsoleScreenBufferInfo(stdout, &mut csbi) != 0 {
        return Some((csbi.srWindow.Right - csbi.srWindow.Left) as usize);
      }

      // On mintty/msys/cygwin based terminals, the above fails with
      // INVALID_HANDLE_VALUE. Use an alternate method which works
      // in that case as well.
      let h = CreateFileA(
        "CONOUT$\0".as_ptr() as *const CHAR,
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        ptr::null_mut(),
        OPEN_EXISTING,
        0,
        ptr::null_mut(),
      );
      if h == INVALID_HANDLE_VALUE {
        return None;
      }

      let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
      let rc = GetConsoleScreenBufferInfo(h, &mut csbi);
      CloseHandle(h);
      if rc != 0 {
        let width = (csbi.srWindow.Right - csbi.srWindow.Left) as usize;
        // Unfortunately cygwin/mintty does not set the size of the
        // backing console to match the actual window size. This
        // always reports a size of 80 or 120 (not sure what
        // determines that). Use a conservative max of 60 which should
        // work in most circumstances. ConEmu does some magic to
        // resize the console correctly, but there's no reasonable way
        // to detect which kind of terminal we are running in, or if
        // GetConsoleScreenBufferInfo returns accurate information.
        return Some(cmp::min(60, width));
      }
      None
    }
  }
}

#[cfg(any(
  all(unix, not(any(target_os = "linux", target_os = "macos"))),
  windows
))]
fn default_err_erase_line(shell: &mut Shell) {
  if let Some(max_width) = imp::stderr_width() {
    let blank = " ".repeat(max_width);
    drop(write!(shell.err.as_write(), "{}\r", blank));
  }
}
