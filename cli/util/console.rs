// Copyright 2018-2025 the Deno authors. MIT license.

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
