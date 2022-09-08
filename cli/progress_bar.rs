// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ProgressBar(Arc<Mutex<ProgressBarInner>>);

#[derive(Debug)]
struct ProgressBarInner {
  pb: Option<indicatif::ProgressBar>,
  is_tty: bool,
}

impl Default for ProgressBarInner {
  fn default() -> Self {
    Self {
      pb: None,
      is_tty: colors::is_tty(),
    }
  }
}

impl ProgressBar {
  fn create() -> indicatif::ProgressBar {
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
    pb
  }

  pub fn update(&self, msg: &str) {
    let mut inner = self.0.lock();

    // If we're not running in TTY we're just gonna fallback
    // to using logger crate.
    if !inner.is_tty {
      log::log!(log::Level::Info, "{} {}", colors::green("Download"), msg);
      return;
    }

    let progress_bar = match inner.pb.as_ref() {
      Some(pb) => pb,
      None => {
        let pb = Self::create();
        inner.pb = Some(pb);
        inner.pb.as_ref().unwrap()
      }
    };
    progress_bar.set_message(msg.to_string());
  }

  pub fn finish(&self) {
    let mut inner = self.0.lock();

    match inner.pb.as_ref() {
      Some(pb) => {
        pb.finish_and_clear();
        inner.pb = None;
      }
      None => {}
    };
  }
}
