// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;

use self::draw_thread::DrawThread;
use self::draw_thread::ProgressBarEntry;
use self::draw_thread::ProgressBarEntryStyle;

use super::console::console_size;

mod draw_thread;
mod renderer;

pub struct UpdateGuard {
  maybe_entry: Option<ProgressBarEntry>,
}

impl Drop for UpdateGuard {
  fn drop(&mut self) {
    if let Some(entry) = &self.maybe_entry {
      entry.finish();
    }
  }
}

pub struct UpdateGuardWithProgress(UpdateGuard);

impl UpdateGuardWithProgress {
  pub fn update(&self, value: u64) {
    if let Some(entry) = &self.0.maybe_entry {
      entry.set_position(value);
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarStyle {
  DownloadBars,
  TextOnly,
}

#[derive(Clone, Debug)]
pub struct ProgressBar {
  draw_thread: Option<DrawThread>,
}

impl ProgressBar {
  /// Checks if progress bars are supported
  pub fn are_supported() -> bool {
    atty::is(atty::Stream::Stderr)
      && console_size().is_some()
      && log::log_enabled!(log::Level::Info)
  }

  pub fn new(style: ProgressBarStyle) -> Self {
    Self {
      draw_thread: match Self::are_supported() {
        true => Some(DrawThread::new(match style {
          ProgressBarStyle::DownloadBars => {
            Box::new(renderer::BarProgressBarRenderer)
          }
          ProgressBarStyle::TextOnly => {
            Box::new(renderer::TextOnlyProgressBarRenderer)
          }
        })),
        false => None,
      },
    }
  }

  pub fn is_enabled(&self) -> bool {
    self.draw_thread.is_some()
  }

  pub fn update(&self, msg: &str) -> UpdateGuard {
    self.update_inner(msg.to_string(), ProgressBarEntryStyle::action())
  }

  pub fn update_with_progress(
    &self,
    msg: String,
    total_size: u64,
  ) -> UpdateGuardWithProgress {
    UpdateGuardWithProgress(
      self.update_inner(msg, ProgressBarEntryStyle::download(total_size)),
    )
  }

  fn update_inner(
    &self,
    msg: String,
    style: ProgressBarEntryStyle,
  ) -> UpdateGuard {
    match &self.draw_thread {
      Some(draw_thread) => {
        let entry = draw_thread.add_entry(msg.to_string(), style);
        UpdateGuard {
          maybe_entry: Some(entry),
        }
      }
      None => {
        // if we're not running in TTY, fallback to using logger crate
        if !msg.is_empty() {
          log::log!(log::Level::Info, "{} {}", colors::green("Download"), msg);
        }
        return UpdateGuard { maybe_entry: None };
      }
    }
  }

  pub fn clear_guard(&self) -> ClearGuard {
    if let Some(draw_thread) = &self.draw_thread {
      draw_thread.increment_clear();
    }
    ClearGuard { pb: self.clone() }
  }

  fn decrement_clear(&self) {
    if let Some(draw_thread) = &self.draw_thread {
      draw_thread.decrement_clear();
    }
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
