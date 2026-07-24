// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io;
use std::sync::Arc;

use console_static_text::ConsoleStaticText;
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal;
use deno_core::parking_lot::Mutex;
use deno_runtime::ops::tty::ConsoleSize;

use super::draw_thread::DrawThread;

/// Gets the console size.
pub fn console_size() -> Option<ConsoleSize> {
  deno_runtime::ops::tty::console_size_of_stderr().ok()
}

pub fn new_console_static_text() -> ConsoleStaticText {
  ConsoleStaticText::new(move || {
    let size = console_size();
    let to_u16 = |value: u32| value.min(u16::MAX as u32) as u16;
    console_static_text::ConsoleSize {
      cols: size.map(|size| size.cols).map(to_u16),
      rows: size.map(|size| size.rows).map(to_u16),
    }
  })
}

/// Strips destructive terminal control characters from user output while
/// preserving ordinary text, whitespace (tab and newline), and SGR (color/
/// style) sequences.
///
/// This covers the full C0 control range and DEL, the C1 controls (raw and
/// UTF-8-encoded), and ANSI escape sequences (CSI/OSC/DCS/PM/APC). Only TAB,
/// LF, and `\r\n` line endings are kept from the control ranges; a bare `\r`
/// (line-overwrite) is stripped. Returns `Cow::Borrowed` when no filtering is
/// needed.
pub fn filter_destructive_ansi(input: &[u8]) -> Cow<'_, [u8]> {
  // Trigger the filter for any control byte other than TAB (0x09) and LF
  // (0x0a), plus C1 controls and the `0xc2` UTF-8 C1 lead. Everything else is
  // copied through unchanged.
  if !input.iter().any(
    |&b| matches!(b, 0x00..=0x08 | 0x0b..=0x1f | 0x7f | 0x80..=0x9f | 0xc2),
  ) {
    return Cow::Borrowed(input);
  }

  let mut out = Vec::with_capacity(input.len());
  let mut i = 0;

  while i < input.len() {
    match input[i] {
      // Strip destructive C0 controls (BEL, BS, ENQ answerback, VT, FF, SI/SO,
      // XON/XOFF flow control, CAN, SUB, separators, ...) and DEL. TAB (0x09)
      // and LF (0x0a) fall through to the default arm and are preserved; CR
      // (0x0d) and ESC (0x1b) are handled by the arms just below.
      0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1a | 0x1c..=0x1f | 0x7f => i += 1,
      // Strip standalone \r (line-overwrite), keep \r\n
      b'\r' if i + 1 < input.len() && input[i + 1] == b'\n' => {
        out.extend_from_slice(b"\r\n");
        i += 2;
      }
      b'\r' => i += 1,
      0x1b if i + 1 >= input.len() => i += 1,
      0x1b => {
        match input[i + 1] {
          b'[' => {
            let seq_end = skip_csi(&input[i..]);
            // Keep SGR sequences (final byte 'm', no private marker '?'/'>'/'<')
            let final_byte = input.get(i + seq_end - 1);
            let has_private = input
              .get(i + 2)
              .is_some_and(|&b| matches!(b, b'?' | b'>' | b'<'));
            if final_byte == Some(&b'm') && !has_private {
              out.extend_from_slice(&input[i..i + seq_end]);
            }
            i += seq_end;
          }
          // OSC/DCS/PM/APC: string sequences terminated by BEL/ST
          b']' | b'P' | b'^' | b'_' => i += skip_str_seq(&input[i..]),
          // Two-byte ESC sequences (Fe/Fp/Fs)
          0x30..=0x7E => i += 2,
          // nF: ESC + intermediate bytes (0x20..=0x2F) + final byte
          0x20..=0x2F => {
            i += 2;
            while i < input.len() && (0x20..=0x2F).contains(&input[i]) {
              i += 1;
            }
            if i < input.len() && (0x30..=0x7E).contains(&input[i]) {
              i += 1;
            }
          }
          _ => i += 1,
        }
      }
      // Strip UTF-8 encoded C1 control characters. Some terminals interpret
      // U+009B as CSI and related C1 controls as terminal control sequences.
      0xc2 if input.get(i + 1).is_some_and(|b| (0x80..=0x9f).contains(b)) => {
        match input[i + 1] {
          0x90 | 0x9d | 0x9e | 0x9f => {
            i += skip_str_seq_after_intro(&input[i..], 2)
          }
          0x9b => i += skip_csi_after_intro(&input[i..], 2),
          _ => i += 2,
        }
      }
      // Preserve valid UTF-8 multibyte sequences as a unit so continuation
      // bytes in normal Unicode output are not mistaken for raw C1 controls.
      0xc2..=0xf4 => {
        if let Some(len) = utf8_sequence_len(&input[i..]) {
          out.extend_from_slice(&input[i..i + len]);
          i += len;
        } else {
          out.push(input[i]);
          i += 1;
        }
      }
      // Strip raw C1 control characters for byte output that is not UTF-8.
      0x90 | 0x9d | 0x9e | 0x9f => {
        i += skip_str_seq_after_intro(&input[i..], 1)
      }
      0x9b => i += skip_csi_after_intro(&input[i..], 1),
      0x80..=0x9f => i += 1,
      b => {
        out.push(b);
        i += 1;
      }
    }
  }

  Cow::Owned(out)
}

/// Returns the length of a CSI sequence (`ESC [` params final-byte).
fn skip_csi(data: &[u8]) -> usize {
  skip_csi_after_intro(data, 2)
}

/// Returns the length of a CSI sequence after a one or two byte introducer.
fn skip_csi_after_intro(data: &[u8], mut j: usize) -> usize {
  if j < data.len() && matches!(data[j], b'?' | b'>' | b'<') {
    j += 1;
  }
  while j < data.len() && (0x30..=0x3F).contains(&data[j]) {
    j += 1;
  }
  while j < data.len() && (0x20..=0x2F).contains(&data[j]) {
    j += 1;
  }
  if j < data.len() && (0x40..=0x7E).contains(&data[j]) {
    j += 1;
  }
  j
}

/// Skips an OSC/DCS/PM/APC string sequence terminated by BEL, ST (ESC \), or 0x9c.
fn skip_str_seq(data: &[u8]) -> usize {
  skip_str_seq_after_intro(data, 2)
}

/// Skips a string control sequence after a one or two byte introducer.
fn skip_str_seq_after_intro(data: &[u8], mut j: usize) -> usize {
  while j < data.len() {
    match data[j] {
      0x07 | 0x9c => return j + 1,
      0xc2 if data.get(j + 1) == Some(&0x9c) => return j + 2,
      0x1b if data.get(j + 1) == Some(&b'\\') => return j + 2,
      _ => j += 1,
    }
  }
  j
}

/// Returns the length of the leading, genuinely valid UTF-8 sequence, if any.
///
/// Overlong, surrogate, and out-of-range encodings are rejected (return `None`)
/// so that the caller does not preserve them wholesale. This matters for
/// security: an overlong encoding of a C1 control (e.g. `e0 82 9b` for U+009B /
/// CSI) is not valid UTF-8, and treating it as a unit would let it survive the
/// filter and be acted on by a lenient or 8-bit terminal. Rejecting it here
/// makes those bytes fall through to the C1-stripping arms of the main loop.
fn utf8_sequence_len(data: &[u8]) -> Option<usize> {
  let len = match *data.first()? {
    0xc2..=0xdf => 2,
    0xe0..=0xef => 3,
    0xf0..=0xf4 => 4,
    _ => return None,
  };
  let seq = data.get(..len)?;
  std::str::from_utf8(seq).ok().map(|_| len)
}

pub struct RawMode {
  needs_disable: bool,
  #[cfg(windows)]
  original_mode: Option<u32>,
}

impl RawMode {
  pub fn enable() -> io::Result<Self> {
    terminal::enable_raw_mode()?;

    #[cfg(windows)]
    {
      // Clear ENABLE_VIRTUAL_TERMINAL_INPUT so that arrow keys and
      // special keys are delivered as VK_* key events via
      // ReadConsoleInput, rather than as VT escape sequences.
      // Windows Terminal enables this flag by default, which causes
      // crossterm to miss arrow keys, Enter, and Ctrl+C.
      let original_mode = windows_vt_input::clear_vt_input_flag();
      Ok(Self {
        needs_disable: true,
        original_mode,
      })
    }

    #[cfg(not(windows))]
    Ok(Self {
      needs_disable: true,
    })
  }

  pub fn disable(mut self) -> io::Result<()> {
    self.needs_disable = false;
    #[cfg(windows)]
    windows_vt_input::restore_mode(self.original_mode);
    terminal::disable_raw_mode()
  }
}

impl Drop for RawMode {
  fn drop(&mut self) {
    if self.needs_disable {
      #[cfg(windows)]
      windows_vt_input::restore_mode(self.original_mode);
      let _ = terminal::disable_raw_mode();
    }
  }
}

#[cfg(windows)]
mod windows_vt_input {
  use windows_sys::Win32::System::Console::GetConsoleMode;
  use windows_sys::Win32::System::Console::GetStdHandle;
  use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
  use windows_sys::Win32::System::Console::SetConsoleMode;

  const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

  /// Clear ENABLE_VIRTUAL_TERMINAL_INPUT on stdin and return the
  /// original mode so it can be restored later.
  pub fn clear_vt_input_flag() -> Option<u32> {
    // SAFETY: GetStdHandle/GetConsoleMode/SetConsoleMode are safe
    // Windows API calls with valid handle constants.
    unsafe {
      let handle = GetStdHandle(STD_INPUT_HANDLE);
      if handle.is_null() {
        return None;
      }
      let mut mode: u32 = 0;
      if GetConsoleMode(handle, &mut mode) == 0 {
        return None;
      }
      if mode & ENABLE_VIRTUAL_TERMINAL_INPUT != 0 {
        SetConsoleMode(handle, mode & !ENABLE_VIRTUAL_TERMINAL_INPUT);
        Some(mode)
      } else {
        None // flag wasn't set, nothing to restore
      }
    }
  }

  pub fn restore_mode(original_mode: Option<u32>) {
    if let Some(mode) = original_mode {
      // SAFETY: restoring previously saved console mode.
      unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        if !handle.is_null() {
          SetConsoleMode(handle, mode);
        }
      }
    }
  }
}

/// A snapshot of the terminal mode that can be restored later.
///
/// This is used by `deno task` so that a child process (for example a dev
/// server like `vite`) that switches the terminal into raw mode and is then
/// terminated (e.g. via Ctrl+C) does not leave the user's terminal in a broken
/// state with input echo and line editing disabled.
///
/// On non-Windows platforms this is a no-op: there the child process belongs to
/// the same process group and the controlling terminal's line discipline is
/// restored by the shell, so there's nothing for `deno` to do.
#[derive(Clone, Copy, Default)]
pub struct SavedTerminalMode {
  #[cfg(windows)]
  stdin_mode: Option<u32>,
  #[cfg(windows)]
  stdout_mode: Option<u32>,
}

impl SavedTerminalMode {
  /// Captures the current terminal mode so it can be restored later.
  pub fn capture() -> Self {
    #[cfg(windows)]
    {
      Self {
        stdin_mode: windows_console_mode::get(
          windows_sys::Win32::System::Console::STD_INPUT_HANDLE,
        ),
        stdout_mode: windows_console_mode::get(
          windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE,
        ),
      }
    }
    #[cfg(not(windows))]
    {
      Self {}
    }
  }

  /// Restores the captured terminal mode. Safe to call multiple times.
  pub fn restore(&self) {
    #[cfg(windows)]
    {
      windows_console_mode::set(
        windows_sys::Win32::System::Console::STD_INPUT_HANDLE,
        self.stdin_mode,
      );
      windows_console_mode::set(
        windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE,
        self.stdout_mode,
      );
    }
  }
}

/// Captures the terminal mode on creation and restores it on drop, acting as a
/// backstop so the terminal is always returned to its original state once a
/// task and all of its children have finished. See [`SavedTerminalMode`].
pub struct TerminalModeGuard(SavedTerminalMode);

impl TerminalModeGuard {
  pub fn acquire() -> Self {
    Self(SavedTerminalMode::capture())
  }

  /// Returns a copy of the captured mode so it can also be restored eagerly
  /// (for example as soon as Ctrl+C is received).
  pub fn saved(&self) -> SavedTerminalMode {
    self.0
  }
}

#[cfg(windows)]
impl Drop for TerminalModeGuard {
  fn drop(&mut self) {
    self.0.restore();
  }
}

#[cfg(windows)]
mod windows_console_mode {
  use windows_sys::Win32::System::Console::GetConsoleMode;
  use windows_sys::Win32::System::Console::GetStdHandle;
  use windows_sys::Win32::System::Console::STD_HANDLE;
  use windows_sys::Win32::System::Console::SetConsoleMode;

  /// Reads the console mode for the given std handle. Returns `None` when the
  /// handle is not a console (e.g. redirected to a pipe or file), in which case
  /// there is no mode to restore.
  pub fn get(std_handle: STD_HANDLE) -> Option<u32> {
    // SAFETY: GetStdHandle/GetConsoleMode are safe Windows API calls with
    // valid handle constants.
    unsafe {
      let handle = GetStdHandle(std_handle);
      if handle.is_null() {
        return None;
      }
      let mut mode: u32 = 0;
      if GetConsoleMode(handle, &mut mode) == 0 {
        return None;
      }
      Some(mode)
    }
  }

  pub fn set(std_handle: STD_HANDLE, mode: Option<u32>) {
    let Some(mode) = mode else {
      return;
    };
    // SAFETY: restoring a previously saved console mode.
    unsafe {
      let handle = GetStdHandle(std_handle);
      if !handle.is_null() {
        SetConsoleMode(handle, mode);
      }
    }
  }
}

pub struct HideCursorGuard {
  needs_disable: bool,
}

impl HideCursorGuard {
  pub fn hide() -> io::Result<Self> {
    io::stderr().execute(cursor::Hide)?;
    Ok(Self {
      needs_disable: true,
    })
  }

  pub fn show(mut self) -> io::Result<()> {
    self.needs_disable = false;
    io::stderr().execute(cursor::Show)?;
    Ok(())
  }
}

impl Drop for HideCursorGuard {
  fn drop(&mut self) {
    if self.needs_disable {
      _ = io::stderr().execute(cursor::Show);
    }
  }
}

#[derive(Debug)]
pub struct ConfirmOptions {
  pub message: String,
  pub default: bool,
}

/// Prompts and confirms if a tty.
///
/// Returns `None` when a tty.
pub fn confirm(options: ConfirmOptions) -> Option<bool> {
  #[derive(Debug)]
  struct PromptRenderer {
    options: ConfirmOptions,
    selection: Arc<Mutex<String>>,
  }

  impl super::draw_thread::DrawThreadRenderer for PromptRenderer {
    fn render(&self, _data: &ConsoleSize) -> String {
      let is_yes_default = self.options.default;
      let selection = self.selection.lock();
      format!(
        "{} [{}/{}] {}",
        self.options.message,
        if is_yes_default { "Y" } else { "y" },
        if is_yes_default { "n" } else { "N" },
        *selection,
      )
    }
  }

  if !DrawThread::is_supported() {
    return None;
  }

  let _raw_mode = RawMode::enable().ok()?;
  let _hide_cursor_guard = HideCursorGuard::hide().ok()?;
  let selection = Arc::new(Mutex::new(String::new()));
  let default = options.default;
  // uses a renderer and the draw thread in order to allow
  // displaying other stuff on the draw thread while the prompt
  // is showing
  let renderer = PromptRenderer {
    options,
    selection: selection.clone(),
  };
  let _state = DrawThread::add_entry(Arc::new(renderer));

  let mut selected = default;
  loop {
    let event = crossterm::event::read().ok()?;
    #[allow(clippy::single_match, reason = "more extendable")]
    match event {
      crossterm::event::Event::Key(KeyEvent {
        kind: KeyEventKind::Press,
        code,
        modifiers,
        ..
      }) => match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
        | (KeyCode::Char('q'), KeyModifiers::NONE) => break,
        (KeyCode::Char('y'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
          selected = true;
          *selection.lock() = "Y".to_string();
        }
        (KeyCode::Char('n'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
          selected = false;
          *selection.lock() = "N".to_string();
        }
        (KeyCode::Backspace, _) => {
          selected = default;
          *selection.lock() = "".to_string();
        }
        // l is common for enter in vim keybindings
        (KeyCode::Enter, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
          return Some(selected);
        }
        _ => {}
      },
      _ => {}
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn terminal_mode_guard_acquire_and_drop() {
    // Should not panic whether or not a console is attached (in CI stdin is
    // typically redirected, so the captured mode is `None` and restoring is a
    // no-op). On a real console it captures and restores the mode. The guard
    // is restored when it goes out of scope at the end of this block, and the
    // captured snapshot can be restored eagerly any number of times.
    {
      let guard = TerminalModeGuard::acquire();
      let saved = guard.saved();
      saved.restore();
      saved.restore();
    }
    SavedTerminalMode::capture().restore();
  }

  #[test]
  fn filter_destructive_ansi_plain_text() {
    let input = b"hello world";
    let result = filter_destructive_ansi(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"hello world");
  }

  #[test]
  fn filter_destructive_ansi_preserves_sgr_color() {
    let input = b"\x1b[31mred\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[31mred\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_preserves_complex_sgr() {
    let input = b"\x1b[1;38;2;255;0;0mbold red\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[1;38;2;255;0;0mbold red\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_strips_clear_screen() {
    let input = b"before\x1b[2Jafter";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_cursor_up() {
    let input = b"line\x1b[2Aup";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"lineup");
  }

  #[test]
  fn filter_destructive_ansi_strips_erase_line() {
    let input = b"text\x1b[Kmore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_strips_cursor_position() {
    let input = b"start\x1b[10;20Hend";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"startend");
  }

  #[test]
  fn filter_destructive_ansi_strips_terminal_reset() {
    let input = b"before\x1bcafter";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_private_mode() {
    // Hide cursor
    let input = b"text\x1b[?25lmore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_strips_osc_title() {
    let input = b"before\x1b]0;evil title\x07after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_osc_with_st() {
    let input = b"before\x1b]0;title\x1b\\after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_mixed_sequences() {
    // SGR red + clear screen + text + SGR reset
    let input = b"\x1b[31mred\x1b[2Jtext\x1b[0m";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[31mredtext\x1b[0m");
  }

  #[test]
  fn filter_destructive_ansi_bel_and_bs_stripped() {
    let input = b"hello\x07world\x08!";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"helloworld!");
  }

  #[test]
  fn filter_destructive_ansi_preserves_whitespace() {
    let input = b"line1\nline2\ttab";
    let result = filter_destructive_ansi(input);
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"line1\nline2\ttab");
  }

  #[test]
  fn filter_destructive_ansi_preserves_utf8_text() {
    let input = "hello 😀 café\n".as_bytes();
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, input);
  }

  #[test]
  fn filter_destructive_ansi_strips_standalone_cr() {
    // Standalone \r (used by progress bars to overwrite lines) is stripped
    let input = b"progress\roverwrite";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"progressoverwrite");
  }

  #[test]
  fn filter_destructive_ansi_preserves_crlf() {
    // \r\n line endings are preserved
    let input = b"line1\r\nline2\r\n";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"line1\r\nline2\r\n");
  }

  #[test]
  fn filter_destructive_ansi_strips_alt_screen() {
    // Alt screen on and off
    let input = b"\x1b[?1049hcontent\x1b[?1049l";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"content");
  }

  #[test]
  fn filter_destructive_ansi_strips_dcs_sequence() {
    let input = b"before\x1bPsome data\x1b\\after";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_scroll_up() {
    let input = b"text\x1b[3Smore";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"textmore");
  }

  #[test]
  fn filter_destructive_ansi_strips_c1_csi_sequences() {
    let utf8_c1_csi = b"before\xc2\x9b2Jafter";
    let result = filter_destructive_ansi(utf8_c1_csi);
    assert_eq!(&*result, b"beforeafter");

    let raw_c1_csi = b"before\x9b2Jafter";
    let result = filter_destructive_ansi(raw_c1_csi);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_c1_string_sequences() {
    let utf8_c1_osc = b"before\xc2\x9d0;evil title\x07after";
    let result = filter_destructive_ansi(utf8_c1_osc);
    assert_eq!(&*result, b"beforeafter");

    let raw_c1_osc = b"before\x9d0;evil title\x07after";
    let result = filter_destructive_ansi(raw_c1_osc);
    assert_eq!(&*result, b"beforeafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_overlong_c1_controls() {
    // Overlong UTF-8 encodings of C1 controls are invalid UTF-8 and must not be
    // preserved as multibyte text: a lenient or 8-bit terminal would otherwise
    // decode `e0 82 9b` as U+009B (CSI) and act on the trailing `2J` (erase
    // screen). The overlong lead byte is emitted as an inert invalid byte while
    // the C1 introducer and its parameters are stripped.
    let overlong_csi = b"before\xe0\x82\x9b2Jafter";
    let result = filter_destructive_ansi(overlong_csi);
    assert_eq!(&*result, b"before\xe0after");

    // The 4-byte overlong form of the same control is likewise defanged (here a
    // `CSI 6 n` cursor-position report, which would otherwise write to stdin).
    let overlong_csi_4 = b"x\xf0\x80\x82\x9b6nend";
    let result = filter_destructive_ansi(overlong_csi_4);
    assert_eq!(&*result, b"x\xf0end");
  }

  #[test]
  fn filter_destructive_ansi_strips_shift_in_out() {
    let input = b"before\x0eshifted\x0fafter";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"beforeshiftedafter");
  }

  #[test]
  fn filter_destructive_ansi_strips_c0_controls() {
    // ENQ (answerback -> stdin writeback), NUL, VT, FF, XON/XOFF flow control,
    // CAN, SUB, the separators, and DEL are all stripped.
    let input = b"a\x00b\x05c\x0bd\x0ce\x11f\x13g\x18h\x1ai\x1cj\x1fk\x7fl";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"abcdefghijkl");
  }

  #[test]
  fn filter_destructive_ansi_preserves_tab_and_newline_around_controls() {
    // TAB and LF are legitimate formatting and must survive even when other
    // control bytes force the slow path.
    let input = b"col1\tcol2\x05\nnext\tline\n";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"col1\tcol2\nnext\tline\n");
  }

  #[test]
  fn filter_destructive_ansi_trailing_esc() {
    let input = b"text\x1b";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"text");
  }

  #[test]
  fn filter_destructive_ansi_sgr_reset_bare() {
    // Bare ESC[m is equivalent to ESC[0m
    let input = b"\x1b[mtext";
    let result = filter_destructive_ansi(input);
    assert_eq!(&*result, b"\x1b[mtext");
  }
}
