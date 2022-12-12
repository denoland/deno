// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use console_static_text::ConsoleStaticText;
use deno_core::parking_lot::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use crate::util::console::console_size;
use crate::util::console::hide_cursor;
use crate::util::console::show_cursor;

use super::renderer::ProgressBarRenderer;
use super::renderer::ProgressData;

// Inspired by Indicatif, but this custom implementation allows
// for more control over what's going on under the hood.

#[derive(Clone, Debug)]
pub enum ProgressBarEntryStyle {
  Download {
    pos: Arc<AtomicU64>,
    total_size: u64,
  },
  Action,
}

impl ProgressBarEntryStyle {
  pub fn download(total_size: u64) -> Self {
    Self::Download {
      pos: Default::default(),
      total_size,
    }
  }

  pub fn action() -> Self {
    Self::Action
  }

  pub fn percent(&self) -> f64 {
    match self {
      ProgressBarEntryStyle::Download { pos, total_size } => {
        let pos = pos.load(Ordering::Relaxed) as f64;
        pos / (*total_size as f64)
      }
      ProgressBarEntryStyle::Action => 0f64,
    }
  }
}

#[derive(Clone, Debug)]
pub struct ProgressBarEntry {
  id: usize,
  pub message: String,
  pub style: ProgressBarEntryStyle,
  draw_thread: DrawThread,
}

impl ProgressBarEntry {
  pub fn set_position(&self, new_pos: u64) {
    if let ProgressBarEntryStyle::Download { pos, .. } = &self.style {
      pos.store(new_pos, Ordering::Relaxed);
    }
  }

  pub fn finish(&self) {
    self.draw_thread.finish_entry(self.id);
  }
}

#[derive(Debug)]
struct InternalState {
  start_time: SystemTime,
  // this ensures only one draw thread is running
  drawer_id: usize,
  keep_alive_count: usize,
  has_draw_thread: bool,
  total_entries: usize,
  entries: Vec<ProgressBarEntry>,
  static_text: ConsoleStaticText,
  renderer: Box<dyn ProgressBarRenderer>,
}

#[derive(Clone, Debug)]
pub struct DrawThread {
  state: Arc<Mutex<InternalState>>,
}

impl DrawThread {
  pub fn new(renderer: Box<dyn ProgressBarRenderer>) -> Self {
    Self {
      state: Arc::new(Mutex::new(InternalState {
        start_time: SystemTime::now(),
        drawer_id: 0,
        keep_alive_count: 0,
        has_draw_thread: false,
        total_entries: 0,
        entries: Vec::new(),
        static_text: ConsoleStaticText::new(|| {
          let size = console_size().unwrap();
          console_static_text::ConsoleSize {
            cols: Some(size.cols as u16),
            rows: Some(size.rows as u16),
          }
        }),
        renderer,
      })),
    }
  }

  pub fn add_entry(
    &self,
    message: String,
    style: ProgressBarEntryStyle,
  ) -> ProgressBarEntry {
    let mut internal_state = self.state.lock();
    let id = internal_state.total_entries;
    let entry = ProgressBarEntry {
      id,
      draw_thread: self.clone(),
      message,
      style,
    };
    internal_state.entries.push(entry.clone());
    internal_state.total_entries += 1;
    internal_state.keep_alive_count += 1;

    if !internal_state.has_draw_thread {
      self.start_draw_thread(&mut internal_state);
    }

    entry
  }

  fn finish_entry(&self, entry_id: usize) {
    let mut internal_state = self.state.lock();

    if let Ok(index) = internal_state
      .entries
      .binary_search_by(|e| e.id.cmp(&entry_id))
    {
      internal_state.entries.remove(index);
      internal_state.keep_alive_count -= 1;
    }
  }

  pub fn increment_clear(&self) {
    let mut internal_state = self.state.lock();
    internal_state.keep_alive_count += 1;
  }

  pub fn decrement_clear(&self) {
    let mut internal_state = self.state.lock();
    internal_state.keep_alive_count -= 1;

    if internal_state.keep_alive_count == 0 {
      internal_state.static_text.eprint_clear();
      // bump the drawer id to exit the draw thread
      internal_state.drawer_id += 1;
      internal_state.has_draw_thread = false;
      show_cursor();
    }
  }

  fn start_draw_thread(&self, internal_state: &mut InternalState) {
    hide_cursor();
    internal_state.drawer_id += 1;
    internal_state.start_time = SystemTime::now();
    internal_state.has_draw_thread = true;
    let drawer_id = internal_state.drawer_id;
    let internal_state = self.state.clone();
    tokio::task::spawn_blocking(move || {
      let mut previous_size = console_size().unwrap();
      loop {
        let mut delay_ms = 120;
        {
          let mut internal_state = internal_state.lock();
          // exit if not the current draw thread
          if internal_state.drawer_id != drawer_id {
            break;
          }

          let size = console_size().unwrap();
          if size != previous_size {
            // means the user is actively resizing the console...
            // wait a little bit until they stop resizing
            previous_size = size;
            delay_ms = 400;
          } else if !internal_state.entries.is_empty() {
            let text = internal_state.renderer.render(ProgressData {
              duration: internal_state.start_time.elapsed().unwrap(),
              entries: &internal_state.entries,
              terminal_width: size.cols,
              total_entries: internal_state.total_entries,
            });

            internal_state.static_text.eprint_with_size(
              &text,
              console_static_text::ConsoleSize {
                cols: Some(size.cols as u16),
                rows: Some(size.rows as u16),
              },
            );
          }
        }

        std::thread::sleep(Duration::from_millis(delay_ms));
      }
    });
  }
}
