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
  let stderr = &deno_runtime::deno_io::STDERR_HANDLE;
  deno_runtime::ops::tty::console_size(stderr).ok()
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

/// Strips destructive ANSI escape sequences from user output while preserving
/// SGR (color/style) sequences. Returns `Cow::Borrowed` when no filtering needed.
pub fn filter_destructive_ansi(input: &[u8]) -> Cow<'_, [u8]> {
  if !input
    .iter()
    .any(|&b| b == 0x1b || b == 0x07 || b == 0x08 || b == b'\r')
  {
    return Cow::Borrowed(input);
  }

  let mut out = Vec::with_capacity(input.len());
  let mut i = 0;

  while i < input.len() {
    match input[i] {
      0x07 | 0x08 => i += 1,
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
  let mut j = 2;
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
  let mut j = 2;
  while j < data.len() {
    match data[j] {
      0x07 | 0x9c => return j + 1,
      0x1b if data.get(j + 1) == Some(&b'\\') => return j + 2,
      _ => j += 1,
    }
  }
  j
}

pub struct RawMode {
  needs_disable: bool,
}

impl RawMode {
  pub fn enable() -> io::Result<Self> {
    terminal::enable_raw_mode()?;
    Ok(Self {
      needs_disable: true,
    })
  }

  pub fn disable(mut self) -> io::Result<()> {
    self.needs_disable = false;
    terminal::disable_raw_mode()
  }
}

impl Drop for RawMode {
  fn drop(&mut self) {
    if self.needs_disable {
      let _ = terminal::disable_raw_mode();
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
    #[allow(clippy::single_match)]
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
