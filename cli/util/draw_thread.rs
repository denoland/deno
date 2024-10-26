// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use console_static_text::ConsoleStaticText;
use deno_core::parking_lot::Mutex;
use deno_core::unsync::spawn_blocking;
use deno_runtime::ops::tty::ConsoleSize;
use once_cell::sync::Lazy;
use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;

use crate::util::console::console_size;

/// Renders text that will be displayed stacked in a
/// static place on the console.
pub trait DrawThreadRenderer: Send + Sync + std::fmt::Debug {
  fn render(&self, data: &ConsoleSize) -> String;
}

/// Draw thread guard. Keep this alive for the duration
/// that you wish the entry to be drawn for. Once it is
/// dropped, then the entry will be removed from the draw
/// thread.
#[derive(Debug)]
pub struct DrawThreadGuard(u16);

impl Drop for DrawThreadGuard {
  fn drop(&mut self) {
    DrawThread::finish_entry(self.0)
  }
}

#[derive(Debug, Clone)]
struct InternalEntry {
  id: u16,
  renderer: Arc<dyn DrawThreadRenderer>,
}

#[derive(Debug)]
struct InternalState {
  // this ensures only one actual draw thread is running
  drawer_id: usize,
  hide_count: usize,
  has_draw_thread: bool,
  next_entry_id: u16,
  entries: Vec<InternalEntry>,
  static_text: ConsoleStaticText,
}

impl InternalState {
  pub fn should_exit_draw_thread(&self, drawer_id: usize) -> bool {
    self.drawer_id != drawer_id || self.entries.is_empty()
  }
}

static INTERNAL_STATE: Lazy<Arc<Mutex<InternalState>>> = Lazy::new(|| {
  Arc::new(Mutex::new(InternalState {
    drawer_id: 0,
    hide_count: 0,
    has_draw_thread: false,
    entries: Vec::new(),
    next_entry_id: 0,
    static_text: ConsoleStaticText::new(|| {
      let size = console_size().unwrap();
      console_static_text::ConsoleSize {
        cols: Some(size.cols as u16),
        rows: Some(size.rows as u16),
      }
    }),
  }))
});

static IS_TTY_WITH_CONSOLE_SIZE: Lazy<bool> = Lazy::new(|| {
  std::io::stderr().is_terminal()
    && console_size()
      .map(|s| s.cols > 0 && s.rows > 0)
      .unwrap_or(false)
});

/// The draw thread is responsible for rendering multiple active
/// `DrawThreadRenderer`s to stderr. It is global because the
/// concept of stderr in the process is also a global concept.
#[derive(Clone, Debug)]
pub struct DrawThread;

impl DrawThread {
  /// Is using a draw thread supported.
  pub fn is_supported() -> bool {
    // don't put the log level in the lazy because the
    // log level may change as the application runs
    log::log_enabled!(log::Level::Info) && *IS_TTY_WITH_CONSOLE_SIZE
  }

  /// Adds a renderer to the draw thread.
  pub fn add_entry(renderer: Arc<dyn DrawThreadRenderer>) -> DrawThreadGuard {
    let internal_state = &*INTERNAL_STATE;
    let mut internal_state = internal_state.lock();
    let id = internal_state.next_entry_id;
    internal_state.entries.push(InternalEntry { id, renderer });

    if internal_state.next_entry_id == u16::MAX {
      internal_state.next_entry_id = 0;
    } else {
      internal_state.next_entry_id += 1;
    }

    Self::maybe_start_draw_thread(&mut internal_state);

    DrawThreadGuard(id)
  }

  /// Hides the draw thread.
  pub fn hide() {
    let internal_state = &*INTERNAL_STATE;
    let mut internal_state = internal_state.lock();
    internal_state.hide_count += 1;

    Self::clear_and_stop_draw_thread(&mut internal_state);
  }

  /// Shows the draw thread if it was previously hidden.
  pub fn show() {
    let internal_state = &*INTERNAL_STATE;
    let mut internal_state = internal_state.lock();
    if internal_state.hide_count > 0 {
      internal_state.hide_count -= 1;
      if internal_state.hide_count == 0 {
        Self::maybe_start_draw_thread(&mut internal_state);
      }
    }
  }

  fn finish_entry(entry_id: u16) {
    let internal_state = &*INTERNAL_STATE;
    let mut internal_state = internal_state.lock();

    if let Some(index) =
      internal_state.entries.iter().position(|e| e.id == entry_id)
    {
      internal_state.entries.remove(index);

      if internal_state.entries.is_empty() {
        Self::clear_and_stop_draw_thread(&mut internal_state);
      }
    }
  }

  fn clear_and_stop_draw_thread(internal_state: &mut InternalState) {
    if internal_state.has_draw_thread {
      internal_state.static_text.eprint_clear();
      // bump the drawer id to exit the draw thread
      internal_state.drawer_id += 1;
      internal_state.has_draw_thread = false;
    }
  }

  fn maybe_start_draw_thread(internal_state: &mut InternalState) {
    if internal_state.has_draw_thread
      || internal_state.hide_count > 0
      || internal_state.entries.is_empty()
      || !DrawThread::is_supported()
    {
      return;
    }

    internal_state.drawer_id += 1;
    internal_state.has_draw_thread = true;

    let drawer_id = internal_state.drawer_id;
    spawn_blocking(move || {
      let mut previous_size = console_size();
      loop {
        let mut delay_ms = 120;
        {
          // Get the entries to render.
          let entries = {
            let internal_state = &*INTERNAL_STATE;
            let internal_state = internal_state.lock();
            if internal_state.should_exit_draw_thread(drawer_id) {
              break;
            }
            internal_state.entries.clone()
          };

          // this should always be set, but have the code handle
          // it not being for some reason
          let size = console_size();

          // Call into the renderers outside the lock to prevent a potential
          // deadlock between our internal state lock and the renderers
          // internal state lock.
          //
          // Example deadlock if this code didn't do this:
          // 1. Other thread - Renderer - acquired internal lock to update state
          // 2. This thread  - Acquired internal state
          // 3. Other thread - Renderer - drops DrawThreadGuard
          // 4. This thread - Calls renderer.render within internal lock,
          //    which attempts to acquire the other thread's Render's internal
          //    lock causing a deadlock
          let mut text = String::new();
          if size != previous_size {
            // means the user is actively resizing the console...
            // wait a little bit until they stop resizing
            previous_size = size;
            delay_ms = 200;
          } else if let Some(size) = size {
            let mut should_new_line_next = false;
            for entry in entries {
              let new_text = entry.renderer.render(&size);
              if should_new_line_next && !new_text.is_empty() {
                text.push('\n');
              }
              should_new_line_next = !new_text.is_empty();
              text.push_str(&new_text);
            }

            // now reacquire the lock, ensure we should still be drawing, then
            // output the text
            {
              let internal_state = &*INTERNAL_STATE;
              let mut internal_state = internal_state.lock();
              if internal_state.should_exit_draw_thread(drawer_id) {
                break;
              }
              internal_state.static_text.eprint_with_size(
                &text,
                console_static_text::ConsoleSize {
                  cols: Some(size.cols as u16),
                  rows: Some(size.rows as u16),
                },
              );
            }
          }
        }

        std::thread::sleep(Duration::from_millis(delay_ms));
      }
    });
  }
}
