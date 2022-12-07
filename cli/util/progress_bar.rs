// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::parking_lot::Mutex;
use indexmap::IndexSet;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ProgressBar(Arc<Mutex<ProgressBarInner>>);

#[derive(Debug)]
struct ProgressBarInner {
  pb: Option<indicatif::ProgressBar>,
  is_tty: bool,
  in_flight: IndexSet<String>,
}

impl Default for ProgressBarInner {
  fn default() -> Self {
    Self {
      pb: None,
      is_tty: colors::is_tty(),
      in_flight: IndexSet::default(),
    }
  }
}

impl ProgressBarInner {
  fn get_or_create_pb(&mut self) -> indicatif::ProgressBar {
    if let Some(pb) = self.pb.as_ref() {
      return pb.clone();
    }

    let pb = indicatif::ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_prefix("Download");
    pb.set_style(
      indicatif::ProgressStyle::with_template(
        "{prefix:.green} {spinner:.green} {msg}",
      )
      .unwrap()
      .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    self.pb = Some(pb);
    self.pb.as_ref().unwrap().clone()
  }

  fn add_in_flight(&mut self, msg: &str) {
    if self.in_flight.contains(msg) {
      return;
    }

    self.in_flight.insert(msg.to_string());
  }

  /// Returns if removed "in-flight" was last entry and progress
  /// bar needs to be updated.
  fn remove_in_flight(&mut self, msg: &str) -> bool {
    if !self.in_flight.contains(msg) {
      return false;
    }

    let mut is_last = false;
    if let Some(last) = self.in_flight.last() {
      is_last = last == msg;
    }
    self.in_flight.remove(msg);
    is_last
  }

  fn update_progress_bar(&mut self) {
    let pb = self.get_or_create_pb();
    if let Some(msg) = self.in_flight.last() {
      pb.set_message(msg.clone());
    }
  }
}

pub struct UpdateGuard {
  pb: ProgressBar,
  msg: String,
  noop: bool,
}

impl Drop for UpdateGuard {
  fn drop(&mut self) {
    if self.noop {
      return;
    }

    let mut inner = self.pb.0.lock();
    if inner.remove_in_flight(&self.msg) {
      inner.update_progress_bar();
    }
  }
}

impl ProgressBar {
  pub fn update(&self, msg: &str) -> UpdateGuard {
    let mut guard = UpdateGuard {
      pb: self.clone(),
      msg: msg.to_string(),
      noop: false,
    };
    let mut inner = self.0.lock();

    // If we're not running in TTY we're just gonna fallback
    // to using logger crate.
    if !inner.is_tty {
      log::log!(log::Level::Info, "{} {}", colors::green("Download"), msg);
      guard.noop = true;
      return guard;
    }

    inner.add_in_flight(msg);
    inner.update_progress_bar();
    guard
  }

  pub fn clear(&self) {
    let mut inner = self.0.lock();

    if let Some(pb) = inner.pb.as_ref() {
      pb.finish_and_clear();
      inner.pb = None;
    }
  }

  pub fn clear_guard(&self) -> ClearGuard {
    ClearGuard { pb: self.clone() }
  }
}

pub struct ClearGuard {
  pb: ProgressBar,
}

impl Drop for ClearGuard {
  fn drop(&mut self) {
    self.pb.clear();
  }
}
