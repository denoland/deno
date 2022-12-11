// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;

mod dprint {
  use console_static_text::ConsoleStaticText;
  use console_static_text::ConsoleStaticTextOptions;
  use deno_core::parking_lot::Mutex;
  use deno_runtime::colors;
  use std::borrow::Cow;
  use std::sync::atomic::AtomicU64;
  use std::sync::atomic::Ordering;
  use std::sync::Arc;
  use std::time::Duration;
  use std::time::SystemTime;

  use crate::util::console::console_size;
  use crate::util::console::hide_cursor;
  use crate::util::console::show_cursor;
  use crate::util::display::human_download_size;

  // Inspired by Indicatif, but this custom implementation allows for more control over
  // what's going on under the hood.

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

    fn percent(&self) -> f64 {
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
    message: String,
    style: ProgressBarEntryStyle,
    progress_bar: ProgressBar,
  }

  impl ProgressBarEntry {
    pub fn set_position(&self, new_pos: u64) {
      if let ProgressBarEntryStyle::Download { pos, .. } = &self.style {
        pos.store(new_pos, Ordering::Relaxed);
      }
    }

    pub fn finish(&self) {
      self.progress_bar.finish_progress(self.id);
    }
  }

  #[derive(Clone, Debug)]
  pub struct ProgressBar {
    state: Arc<Mutex<InternalState>>,
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
    text: ConsoleStaticText,
  }

  impl ProgressBar {
    /// Checks if progress bars are supported
    pub fn are_supported() -> bool {
      atty::is(atty::Stream::Stderr)
        && console_size().is_some()
        && log::log_enabled!(log::Level::Info)
    }

    /// Creates a new ProgressBar.
    pub fn new() -> Self {
      ProgressBar {
        state: Arc::new(Mutex::new(InternalState {
          start_time: SystemTime::now(),
          drawer_id: 0,
          keep_alive_count: 0,
          has_draw_thread: false,
          total_entries: 0,
          entries: Vec::new(),
          text: ConsoleStaticText::new(ConsoleStaticTextOptions {
            terminal_width: Box::new(|| console_size().unwrap().cols as u16),
            strip_ansi_codes: Box::new(|text| {
              strip_ansi_escapes::strip(&text)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(Cow::Owned)
                .unwrap_or(Cow::Borrowed(text))
            }),
          }),
        })),
      }
    }

    pub fn add_progress(
      &self,
      message: String,
      style: ProgressBarEntryStyle,
    ) -> ProgressBarEntry {
      let mut internal_state = self.state.lock();
      let id = internal_state.total_entries;
      let pb = ProgressBarEntry {
        id,
        progress_bar: self.clone(),
        message,
        style,
      };
      internal_state.entries.push(pb.clone());
      internal_state.total_entries += 1;
      internal_state.keep_alive_count += 1;

      if !internal_state.has_draw_thread {
        self.start_draw_thread(&mut internal_state);
      }

      pb
    }

    fn finish_progress(&self, progress_bar_id: usize) {
      let mut internal_state = self.state.lock();

      if let Some(index) = internal_state
        .entries
        .iter()
        .position(|p| p.id == progress_bar_id)
      {
        internal_state.entries.remove(index);
      }

      internal_state.keep_alive_count -= 1;
    }

    pub fn increment_clear(&self) {
      let mut internal_state = self.state.lock();
      internal_state.keep_alive_count += 1;
    }

    pub fn decrement_clear(&self) {
      let mut internal_state = self.state.lock();
      internal_state.keep_alive_count -= 1;

      if internal_state.keep_alive_count == 0 {
        internal_state.text.eprint_clear();
        // bump the drawer id to exit the draw thread
        internal_state.drawer_id += 1;
        internal_state.has_draw_thread = false;
        show_cursor();
      }
    }

    fn start_draw_thread(&self, internal_state: &mut InternalState) {
      hide_cursor();
      internal_state.drawer_id += 1;
      internal_state.total_entries = 1; // reset
      internal_state.start_time = SystemTime::now();
      internal_state.has_draw_thread = true;
      let drawer_id = internal_state.drawer_id;
      let internal_state = self.state.clone();
      tokio::task::spawn_blocking(move || {
        loop {
          {
            let mut internal_state = internal_state.lock();
            // exit if not the current draw thread
            if internal_state.drawer_id != drawer_id {
              break;
            }

            if !internal_state.entries.is_empty() {
              // prefer displaying download entries because they have more activity
              let displayed_entry = internal_state
                .entries
                .iter()
                .filter(|e| {
                  matches!(e.style, ProgressBarEntryStyle::Download { .. })
                })
                .next()
                .unwrap_or(&internal_state.entries[0]);

              let mut total_percent = 0f64;
              for entry in &internal_state.entries {
                total_percent += entry.style.percent();
              }
              total_percent += (internal_state.total_entries
                - internal_state.entries.len())
                as f64;
              let percent_done =
                total_percent / (internal_state.total_entries as f64);

              let terminal_width = console_size().unwrap().cols;
              let text = get_progress_bar_text(
                terminal_width,
                &displayed_entry,
                percent_done,
                internal_state.entries.len(),
                internal_state.total_entries,
                internal_state.start_time.elapsed().unwrap(),
              );

              internal_state
                .text
                .eprint_with_width(&text, terminal_width as u16);
            }
          }

          std::thread::sleep(Duration::from_millis(120));
        }
      });
    }
  }

  fn get_progress_bar_text(
    terminal_width: u32,
    display_entry: &ProgressBarEntry,
    total_percent: f64,
    remaining_entries: usize,
    total_entries: usize,
    duration: Duration,
  ) -> String {
    let (bytes_text, bytes_text_max_width) = match &display_entry.style {
      ProgressBarEntryStyle::Download { pos, total_size } => {
        let total_size_str = human_download_size(*total_size, *total_size);
        (
          format!(
            " {}/{}",
            human_download_size(pos.load(Ordering::Relaxed), *total_size),
            total_size_str,
          ),
          2 + total_size_str.len() * 2,
        )
      }
      ProgressBarEntryStyle::Action => (String::new(), 0),
    };
    let (total_text, total_text_max_width) = if total_entries <= 1 {
      (String::new(), 0)
    } else {
      let total_entries_str = total_entries.to_string();
      (
        format!(" ({}/{})", total_entries - remaining_entries, total_entries),
        4 + total_entries_str.len() * 2,
      )
    };

    let elapsed_text = get_elapsed_text(duration);
    let mut text = String::new();
    if !display_entry.message.is_empty() {
      text.push_str(&format!(
        "{} {}{}\n",
        colors::green("Download"),
        display_entry.message,
        bytes_text,
      ));
    }
    text.push_str(&elapsed_text);
    let total_bars = (std::cmp::min(75, terminal_width - 5) as usize)
      - elapsed_text.len()
      - total_text_max_width
      - bytes_text_max_width
      - 3; // space, open and close brace
    let completed_bars = (total_bars as f64 * total_percent).floor() as usize;
    text.push_str(" [");
    if completed_bars != total_bars {
      if completed_bars > 0 {
        text.push_str(&format!(
          "{}",
          colors::cyan(format!("{}{}", "#".repeat(completed_bars - 1), ">"))
        ))
      }
      text.push_str(&format!(
        "{}",
        colors::intense_blue("-".repeat(total_bars - completed_bars))
      ))
    } else {
      text.push_str(&format!("{}", colors::cyan("#".repeat(completed_bars))))
    }
    text.push(']');

    // suffix
    if display_entry.message.is_empty() {
      text.push_str(&bytes_text);
    }
    text.push_str(&total_text);

    text
  }

  fn get_elapsed_text(elapsed: Duration) -> String {
    let elapsed_secs = elapsed.as_secs();
    let seconds = elapsed_secs % 60;
    let minutes = elapsed_secs / 60;
    format!("[{:0>2}:{:0>2}]", minutes, seconds)
  }

  #[cfg(test)]
  mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn should_get_elapsed_text() {
      assert_eq!(get_elapsed_text(Duration::from_secs(1)), "[00:01]");
      assert_eq!(get_elapsed_text(Duration::from_secs(20)), "[00:20]");
      assert_eq!(get_elapsed_text(Duration::from_secs(59)), "[00:59]");
      assert_eq!(get_elapsed_text(Duration::from_secs(60)), "[01:00]");
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 5 + 23)),
        "[05:23]"
      );
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 59 + 59)),
        "[59:59]"
      );
      assert_eq!(get_elapsed_text(Duration::from_secs(60 * 60)), "[01:00:00]");
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 60 * 3 + 20 * 60 + 2)),
        "[200:02]"
      );
      assert_eq!(
        get_elapsed_text(Duration::from_secs(60 * 60 * 99)),
        "[5940:00]"
      );
    }
  }
}
#[derive(Clone, Debug)]
pub struct ProgressBar {
  pb: Option<dprint::ProgressBar>,
}

impl Default for ProgressBar {
  fn default() -> Self {
    Self {
      pb: match dprint::ProgressBar::are_supported() {
        true => Some(dprint::ProgressBar::new()),
        false => None,
      },
    }
  }
}

pub struct UpdateGuard {
  maybe_entry: Option<dprint::ProgressBarEntry>,
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

impl ProgressBar {
  pub fn is_enabled(&self) -> bool {
    self.pb.is_some()
  }

  pub fn update(&self, msg: &str) -> UpdateGuard {
    self.update_inner(msg.to_string(), dprint::ProgressBarEntryStyle::action())
  }

  pub fn update_with_progress(
    &self,
    msg: String,
    total_size: u64,
  ) -> UpdateGuardWithProgress {
    UpdateGuardWithProgress(
      self
        .update_inner(msg, dprint::ProgressBarEntryStyle::download(total_size)),
    )
  }

  fn update_inner(
    &self,
    msg: String,
    style: dprint::ProgressBarEntryStyle,
  ) -> UpdateGuard {
    match &self.pb {
      Some(pb) => {
        let entry = pb.add_progress(msg.to_string(), style);
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
    if let Some(pb) = &self.pb {
      pb.increment_clear();
    }
    ClearGuard { pb: self.clone() }
  }

  fn decrement_clear(&self) {
    if let Some(pb) = &self.pb {
      pb.decrement_clear();
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
