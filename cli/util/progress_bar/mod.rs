// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use deno_core::parking_lot::Mutex;
use deno_runtime::ops::tty::ConsoleSize;

use crate::colors;

use self::renderer::ProgressBarRenderer;
use self::renderer::ProgressData;
use self::renderer::ProgressDataDisplayEntry;

use super::draw_thread::DrawThread;
use super::draw_thread::DrawThreadGuard;
use super::draw_thread::DrawThreadRenderer;

mod renderer;

// Inspired by Indicatif, but this custom implementation allows
// for more control over what's going on under the hood.

#[derive(Debug, Clone, Copy)]
pub enum ProgressMessagePrompt {
  Download,
  Blocking,
  Initialize,
  Cleaning,
}

impl ProgressMessagePrompt {
  pub fn as_text(&self) -> String {
    match self {
      ProgressMessagePrompt::Download => colors::green("Download").to_string(),
      ProgressMessagePrompt::Blocking => colors::cyan("Blocking").to_string(),
      ProgressMessagePrompt::Initialize => {
        colors::green("Initialize").to_string()
      }
      ProgressMessagePrompt::Cleaning => colors::green("Cleaning").to_string(),
    }
  }
}

#[derive(Debug)]
pub struct UpdateGuard {
  maybe_entry: Option<Arc<ProgressBarEntry>>,
}

impl Drop for UpdateGuard {
  fn drop(&mut self) {
    if let Some(entry) = &self.maybe_entry {
      entry.finish();
    }
  }
}

impl UpdateGuard {
  pub fn set_position(&self, value: u64) {
    if let Some(entry) = &self.maybe_entry {
      entry.set_position(value);
    }
  }

  pub fn set_total_size(&self, value: u64) {
    if let Some(entry) = &self.maybe_entry {
      entry.set_total_size(value);
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarStyle {
  /// Shows a progress bar with human readable download size
  DownloadBars,

  /// Shows a progress bar with numeric progres count
  ProgressBars,

  /// Shows a list of currently downloaded files.
  TextOnly,
}

#[derive(Debug)]
struct ProgressBarEntry {
  id: usize,
  prompt: ProgressMessagePrompt,
  pub message: String,
  pos: AtomicU64,
  total_size: AtomicU64,
  progress_bar: ProgressBarInner,
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
    self.progress_bar.finish_entry(self.id);
  }

  pub fn percent(&self) -> f64 {
    let pos = self.pos.load(Ordering::Relaxed) as f64;
    let total_size = self.total_size.load(Ordering::Relaxed) as f64;
    if total_size == 0f64 {
      0f64
    } else if pos > total_size {
      1f64
    } else {
      pos / total_size
    }
  }
}

#[derive(Debug)]
struct InternalState {
  /// If this guard exists, then it means the progress
  /// bar is displaying in the draw thread.
  draw_thread_guard: Option<DrawThreadGuard>,
  start_time: Instant,
  keep_alive_count: usize,
  total_entries: usize,
  entries: Vec<Arc<ProgressBarEntry>>,
}

#[derive(Clone, Debug)]
struct ProgressBarInner {
  state: Arc<Mutex<InternalState>>,
  renderer: Arc<dyn ProgressBarRenderer>,
}

impl ProgressBarInner {
  fn new(renderer: Arc<dyn ProgressBarRenderer>) -> Self {
    Self {
      state: Arc::new(Mutex::new(InternalState {
        draw_thread_guard: None,
        start_time: Instant::now(),
        keep_alive_count: 0,
        total_entries: 0,
        entries: Vec::new(),
      })),
      renderer,
    }
  }

  pub fn add_entry(
    &self,
    kind: ProgressMessagePrompt,
    message: String,
  ) -> Arc<ProgressBarEntry> {
    let mut internal_state = self.state.lock();
    let id = internal_state.total_entries;
    let entry = Arc::new(ProgressBarEntry {
      id,
      prompt: kind,
      message,
      pos: Default::default(),
      total_size: Default::default(),
      progress_bar: self.clone(),
    });
    internal_state.entries.push(entry.clone());
    internal_state.total_entries += 1;
    internal_state.keep_alive_count += 1;

    self.maybe_start_draw_thread(&mut internal_state);

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

  fn decrement_keep_alive(&self, state: &mut InternalState) {
    state.keep_alive_count -= 1;

    if state.keep_alive_count == 0 {
      // drop the guard to remove this from the draw thread
      state.draw_thread_guard.take();
    }
  }

  fn maybe_start_draw_thread(&self, internal_state: &mut InternalState) {
    if internal_state.draw_thread_guard.is_none()
      && internal_state.keep_alive_count > 0
    {
      internal_state.start_time = Instant::now();
      internal_state.draw_thread_guard =
        Some(DrawThread::add_entry(Arc::new(self.clone())));
    }
  }
}

impl DrawThreadRenderer for ProgressBarInner {
  fn render(&self, size: &ConsoleSize) -> String {
    let data = {
      let state = self.state.lock();
      if state.entries.is_empty() {
        return String::new();
      }
      let display_entries = state
        .entries
        .iter()
        .map(|e| ProgressDataDisplayEntry {
          prompt: e.prompt,
          message: e.message.to_string(),
          position: e.position(),
          total_size: e.total_size(),
        })
        .collect::<Vec<_>>();

      ProgressData {
        duration: state.start_time.elapsed(),
        terminal_width: size.cols,
        pending_entries: state.entries.len(),
        total_entries: state.total_entries,
        display_entries,
        percent_done: {
          let mut total_percent_sum = 0f64;
          for entry in &state.entries {
            total_percent_sum += entry.percent();
          }
          total_percent_sum +=
            (state.total_entries - state.entries.len()) as f64;
          total_percent_sum / (state.total_entries as f64)
        },
      }
    };
    self.renderer.render(data)
  }
}

#[derive(Clone, Debug)]
pub struct ProgressBar {
  inner: ProgressBarInner,
}

impl ProgressBar {
  /// Checks if progress bars are supported
  pub fn are_supported() -> bool {
    DrawThread::is_supported()
  }

  pub fn new(style: ProgressBarStyle) -> Self {
    Self {
      inner: ProgressBarInner::new(match style {
        ProgressBarStyle::DownloadBars => {
          Arc::new(renderer::BarProgressBarRenderer {
            display_human_download_size: true,
          })
        }
        ProgressBarStyle::ProgressBars => {
          Arc::new(renderer::BarProgressBarRenderer {
            display_human_download_size: false,
          })
        }
        ProgressBarStyle::TextOnly => {
          Arc::new(renderer::TextOnlyProgressBarRenderer::default())
        }
      }),
    }
  }

  pub fn update(&self, msg: &str) -> UpdateGuard {
    self.update_with_prompt(ProgressMessagePrompt::Download, msg)
  }

  pub fn update_with_prompt(
    &self,
    kind: ProgressMessagePrompt,
    msg: &str,
  ) -> UpdateGuard {
    // only check if progress bars are supported once we go
    // to update so that we lazily initialize the progress bar
    if ProgressBar::are_supported() {
      let entry = self.inner.add_entry(kind, msg.to_string());
      UpdateGuard {
        maybe_entry: Some(entry),
      }
    } else {
      // if we're not running in TTY, fallback to using logger crate
      if !msg.is_empty() {
        log::log!(log::Level::Info, "{} {}", kind.as_text(), msg);
      }
      UpdateGuard { maybe_entry: None }
    }
  }

  pub fn clear_guard(&self) -> ClearGuard {
    self.inner.increment_clear();
    ClearGuard { pb: self.clone() }
  }

  fn decrement_clear(&self) {
    self.inner.decrement_clear();
  }
}

pub struct ClearGuard {
  pb: ProgressBar,
}

impl Drop for ClearGuard {
  fn drop(&mut self) {
    self.pb.decrement_clear();
  }
}
