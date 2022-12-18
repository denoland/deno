// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;

use self::draw_thread::DrawThread;
use self::draw_thread::ProgressBarEntry;

use super::console::console_size;

mod draw_thread;
mod renderer;

// Inspired by Indicatif, but this custom implementation allows
// for more control over what's going on under the hood.

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
      && log::log_enabled!(log::Level::Info)
      && console_size()
        .map(|s| s.cols > 0 && s.rows > 0)
        .unwrap_or(false)
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

  pub fn update(&self, msg: &str) -> UpdateGuard {
    match &self.draw_thread {
      Some(draw_thread) => {
        let entry = draw_thread.add_entry(msg.to_string());
        UpdateGuard {
          maybe_entry: Some(entry),
        }
      }
      None => {
        // if we're not running in TTY, fallback to using logger crate
        if !msg.is_empty() {
          log::log!(log::Level::Info, "{} {}", colors::green("Download"), msg);
        }
        UpdateGuard { maybe_entry: None }
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
