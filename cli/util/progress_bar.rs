// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use deno_core::parking_lot::Mutex;
use indexmap::IndexSet;
use std::sync::Arc;
use std::time::Duration;

mod dprint {
  use crossterm::style::Stylize;
  use crossterm::tty::IsTty;
  use deno_core::anyhow::Result;
  use deno_core::parking_lot::RwLock;
  use std::sync::Arc;
  use std::time::Duration;
  use std::time::SystemTime;

  pub fn get_terminal_width() -> Option<u16> {
    get_terminal_size().map(|(cols, _)| cols)
  }

  /// Gets the terminal size (width/cols, height/rows).
  pub fn get_terminal_size() -> Option<(u16, u16)> {
    match crossterm::terminal::size() {
      Ok(size) => Some(size),
      Err(_) => None,
    }
  }

  // Inspired by Indicatif, but this custom implementation allows for more control over
  // what's going on under the hood and it works better with the multi-threading model
  // going on in dprint.

  #[derive(Clone, Copy, PartialEq, Eq)]
  pub enum ProgressBarStyle {
    Download,
    Action,
  }

  #[derive(Clone)]
  pub struct ProgressBar {
    id: usize,
    start_time: SystemTime,
    progress_bars: ProgressBars,
    message: String,
    size: usize,
    style: ProgressBarStyle,
    pos: Arc<RwLock<usize>>,
  }

  impl ProgressBar {
    pub fn set_position(&self, new_pos: usize) {
      let mut pos = self.pos.write();
      *pos = new_pos;
    }

    pub fn finish(&self) {
      self.progress_bars.finish_progress(self.id);
    }
  }

  #[derive(Clone)]
  pub struct ProgressBars {
    state: Arc<RwLock<InternalState>>,
  }

  struct InternalState {
    // this ensures only one draw thread is running
    drawer_id: usize,
    progress_bar_counter: usize,
    progress_bars: Vec<ProgressBar>,
  }

  fn clear_previous_line() {
    eprint!("\x1B[1A\x1B[0J");
  }

  impl ProgressBars {
    /// Checks if progress bars are supported
    pub fn are_supported() -> bool {
      std::io::stderr().is_tty() && get_terminal_width().is_some()
    }

    /// Creates a new ProgressBars or returns None when not supported.
    pub fn new() -> Option<Self> {
      if ProgressBars::are_supported() {
        Some(ProgressBars {
          state: Arc::new(RwLock::new(InternalState {
            drawer_id: 0,
            progress_bar_counter: 0,
            progress_bars: Vec::new(),
          })),
        })
      } else {
        None
      }
    }

    pub fn add_progress(
      &self,
      message: String,
      style: ProgressBarStyle,
      total_size: usize,
    ) -> ProgressBar {
      let mut internal_state = self.state.write();
      let id = internal_state.progress_bar_counter;
      let pb = ProgressBar {
        id,
        progress_bars: self.clone(),
        start_time: SystemTime::now(),
        message,
        size: total_size,
        style,
        pos: Arc::new(RwLock::new(0)),
      };
      internal_state.progress_bars.push(pb.clone());
      internal_state.progress_bar_counter += 1;

      if internal_state.progress_bars.len() == 1 {
        self.start_draw_thread(&mut internal_state);
      }

      pb
    }

    fn finish_progress(&self, progress_bar_id: usize) {
      let mut internal_state = self.state.write();

      if let Some(index) = internal_state
        .progress_bars
        .iter()
        .position(|p| p.id == progress_bar_id)
      {
        internal_state.progress_bars.remove(index);
      }

      // Remove last printed line
      clear_previous_line();
    }

    fn start_draw_thread(&self, internal_state: &mut InternalState) {
      internal_state.drawer_id += 1;
      let drawer_id = internal_state.drawer_id;
      let internal_state = self.state.clone();
      std::thread::spawn(move || {
        loop {
          {
            let internal_state = internal_state.read();
            // exit if not the current draw thread or there are no more progress bars
            if internal_state.drawer_id != drawer_id
              || internal_state.progress_bars.is_empty()
            {
              break;
            }

            let terminal_width = get_terminal_width().unwrap();
            let mut text = String::new();
            let progress_bar = internal_state.progress_bars[0];

            text.push_str(&get_progress_bar_text(
              terminal_width,
              *progress_bar.pos.read(),
              progress_bar.size,
              progress_bar.style,
              progress_bar.start_time.elapsed().unwrap(),
            ));

            eprint!("{}", text);
          }

          std::thread::sleep(Duration::from_millis(100));
        }
      });
    }
  }

  fn get_progress_bar_text(
    terminal_width: u16,
    pos: usize,
    total: usize,
    pb_style: ProgressBarStyle,
    duration: Duration,
  ) -> String {
    let total = std::cmp::max(pos, total); // increase the total when pos > total
    let bytes_text = if pb_style == ProgressBarStyle::Download {
      format!(
        " {}/{}",
        get_bytes_text(pos, total),
        get_bytes_text(total, total)
      )
    } else {
      String::new()
    };

    let elapsed_text = get_elapsed_text(duration);
    let mut text = String::new();
    text.push_str(&elapsed_text);
    // get progress bar
    let percent = pos as f32 / total as f32;
    // don't include the bytes text in this because a string going from X.XXMB to XX.XXMB should not adjust the progress bar
    let total_bars = (std::cmp::min(50, terminal_width - 15) as usize)
      - elapsed_text.len()
      - 1
      - 2;
    let completed_bars = (total_bars as f32 * percent).floor() as usize;
    text.push_str(" [");
    if completed_bars != total_bars {
      if completed_bars > 0 {
        text.push_str(&format!(
          "{}",
          format!("{}{}", "#".repeat(completed_bars - 1), ">").cyan()
        ))
      }
      text.push_str(&format!(
        "{}",
        "-".repeat(total_bars - completed_bars).blue()
      ))
    } else {
      text.push_str(&format!("{}", "#".repeat(completed_bars).cyan()))
    }
    text.push(']');

    // bytes text
    text.push_str(&bytes_text);

    text
  }

  fn get_bytes_text(byte_count: usize, total_bytes: usize) -> String {
    let bytes_to_kb = 1_000;
    let bytes_to_mb = 1_000_000;
    return if total_bytes < bytes_to_mb {
      get_in_format(byte_count, bytes_to_kb, "KB")
    } else {
      get_in_format(byte_count, bytes_to_mb, "MB")
    };

    fn get_in_format(
      byte_count: usize,
      conversion: usize,
      suffix: &str,
    ) -> String {
      let converted_value = byte_count / conversion;
      let decimal = (byte_count % conversion) * 100 / conversion;
      format!("{}.{:0>2}{}", converted_value, decimal, suffix)
    }
  }

  fn get_elapsed_text(elapsed: Duration) -> String {
    let elapsed_secs = elapsed.as_secs();
    let seconds = elapsed_secs % 60;
    let minutes = (elapsed_secs / 60) % 60;
    let hours = (elapsed_secs / 60) / 60;
    format!("[{:0>2}:{:0>2}:{:0>2}]", hours, minutes, seconds)
  }

  #[cfg(test)]
  mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn should_get_bytes_text() {
      assert_eq!(get_bytes_text(9, 999), "0.00KB");
      assert_eq!(get_bytes_text(10, 999), "0.01KB");
      assert_eq!(get_bytes_text(100, 999), "0.10KB");
      assert_eq!(get_bytes_text(200, 999), "0.20KB");
      assert_eq!(get_bytes_text(520, 999), "0.52KB");
      assert_eq!(get_bytes_text(1000, 10_000), "1.00KB");
      assert_eq!(get_bytes_text(10_000, 10_000), "10.00KB");
      assert_eq!(get_bytes_text(999_999, 990_999), "999.99KB");
      assert_eq!(get_bytes_text(1_000_000, 1_000_000), "1.00MB");
      assert_eq!(get_bytes_text(9_524_102, 10_000_000), "9.52MB");
    }

    #[test]
    fn should_get_elapsed_text() {
      assert_eq!(get_elapsed_text(Duration::from_secs(1)), "[00:00:01]");
      assert_eq!(get_elapsed_text(Duration::from_secs(20)), "[00:00:20]");
      assert_eq!(get_elapsed_text(Duration::from_secs(59)), "[00:00:59]");
      assert_eq!(get_elapsed_text(Duration::from_secs(60)), "[00:01:00]");
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 5 + 23)),
        "[00:05:23]"
      );
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 59 + 59)),
        "[00:59:59]"
      );
      assert_eq!(get_elapsed_text(Duration::from_secs(60 * 60)), "[01:00:00]");
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 60 * 3 + 20 * 60 + 2)),
        "[03:20:02]"
      );
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 60 * 99)),
        "[99:00:00]"
      );
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 60 * 120)),
        "[120:00:00]"
      );
    }
  }
}
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
