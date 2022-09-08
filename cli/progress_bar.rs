// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ProgressBar(Arc<Mutex<Option<indicatif::ProgressBar>>>);

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

    let progress_bar = match inner.as_ref() {
      Some(pb) => pb,
      None => {
        // If we're not running in TTY we're just gonna fallback
        // to using logger crate.
        if !colors::is_tty() {
          log::log!(log::Level::Info, "{} {}", colors::green("Download"), msg);
          return;
        }

        let pb = Self::create();
        *inner = Some(pb);
        inner.as_ref().unwrap()
      }
    };
    progress_bar.set_message(msg.to_string());
  }

  pub fn finish(&self) {
    let mut inner = self.0.lock();

    match inner.as_ref() {
      Some(pb) => {
        pb.finish_with_message("finished");
        *inner = None;
      }
      None => {}
    };
  }
}
