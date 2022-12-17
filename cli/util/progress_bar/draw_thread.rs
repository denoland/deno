// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use console_static_text::ConsoleStaticText;
use deno_core::parking_lot::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use crate::util::console::console_size;

use super::renderer::ProgressBarRenderer;
use super::renderer::ProgressData;
use super::renderer::ProgressDataDisplayEntry;

#[derive(Clone, Debug)]
pub struct ProgressBarEntry {
  id: usize,
  pub message: String,
  pos: Arc<AtomicU64>,
  total_size: Arc<AtomicU64>,
  draw_thread: DrawThread,
}

impl ProgressBarEntry {
  pub fn position(&self) -> u64 {
    self.pos.load(Ordering::Relaxed)
  }

  pub fn set_position(&self, new_pos: u64) {
    self.pos.store(new_pos, Ordering::Relaxed);
  }

  pub fn total_size(&self) -> u64 {
    self.total_size.load(Ordering::Relaxed)
  }

  pub fn set_total_size(&self, new_size: u64) {
    self.total_size.store(new_size, Ordering::Relaxed);
  }

  pub fn finish(&self) {
    self.draw_thread.finish_entry(self.id);
  }

  pub fn percent(&self) -> f64 {
    let pos = self.pos.load(Ordering::Relaxed) as f64;
    let total_size = self.total_size.load(Ordering::Relaxed) as f64;
    if total_size == 0f64 {
      0f64
    } else {
      pos / total_size
    }
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

  pub fn add_entry(&self, message: String) -> ProgressBarEntry {
    let mut internal_state = self.state.lock();
    let id = internal_state.total_entries;
    let entry = ProgressBarEntry {
      id,
      draw_thread: self.clone(),
      message,
      pos: Default::default(),
      total_size: Default::default(),
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
      self.decrement_keep_alive(&mut internal_state);
    }
  }

  pub fn increment_clear(&self) {
    let mut internal_state = self.state.lock();
    internal_state.keep_alive_count += 1;
  }

  pub fn decrement_clear(&self) {
    let mut internal_state = self.state.lock();
    self.decrement_keep_alive(&mut internal_state);
  }

  fn decrement_keep_alive(&self, internal_state: &mut InternalState) {
    internal_state.keep_alive_count -= 1;

    if internal_state.keep_alive_count == 0 {
      internal_state.static_text.eprint_clear();
      // bump the drawer id to exit the draw thread
      internal_state.drawer_id += 1;
      internal_state.has_draw_thread = false;
    }
  }

  fn start_draw_thread(&self, internal_state: &mut InternalState) {
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
            delay_ms = 200;
          } else if !internal_state.entries.is_empty() {
            let preferred_entry = internal_state
              .entries
              .iter()
              .find(|e| e.percent() > 0f64)
              .or_else(|| internal_state.entries.iter().last())
              .unwrap();
            let text = internal_state.renderer.render(ProgressData {
              duration: internal_state.start_time.elapsed().unwrap(),
              terminal_width: size.cols,
              pending_entries: internal_state.entries.len(),
              total_entries: internal_state.total_entries,
              display_entry: ProgressDataDisplayEntry {
                message: preferred_entry.message.clone(),
                position: preferred_entry.position(),
                total_size: preferred_entry.total_size(),
              },
              percent_done: {
                let mut total_percent_sum = 0f64;
                for entry in &internal_state.entries {
                  total_percent_sum += entry.percent();
                }
                total_percent_sum += (internal_state.total_entries
                  - internal_state.entries.len())
                  as f64;
                total_percent_sum / (internal_state.total_entries as f64)
              },
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
