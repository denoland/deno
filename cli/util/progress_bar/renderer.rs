// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::time::Duration;

use deno_runtime::colors;

use crate::util::display::human_download_size;

use super::draw_thread::ProgressBarEntry;

pub struct ProgressData<'a> {
  pub terminal_width: u32,
  pub entries: &'a Vec<ProgressBarEntry>,
  pub total_entries: usize,
  pub duration: Duration,
}

impl<'a> ProgressData<'a> {
  pub fn precent_done(&self) -> f64 {
    let mut total_percent_sum = 0f64;
    for entry in self.entries {
      total_percent_sum += entry.percent();
    }
    total_percent_sum += (self.total_entries - self.entries.len()) as f64;
    total_percent_sum / (self.total_entries as f64)
  }

  pub fn preferred_display_entry(&self) -> Option<&ProgressBarEntry> {
    // prefer displaying download entries because they have more activity
    self
      .entries
      .iter()
      .find(|e| e.percent() > 0f64)
      .or_else(|| self.entries.iter().last())
  }
}

pub trait ProgressBarRenderer: Send + std::fmt::Debug {
  fn render(&self, data: ProgressData) -> String;
}

/// Indicatif style progress bar.
#[derive(Debug)]
pub struct BarProgressBarRenderer;

impl ProgressBarRenderer for BarProgressBarRenderer {
  fn render(&self, data: ProgressData) -> String {
    let display_entry = match data.preferred_display_entry() {
      Some(v) => v,
      None => return String::new(),
    };
    let percent_done = data.precent_done();

    let (bytes_text, bytes_text_max_width) = {
      let total_size = display_entry.total_size();
      let pos = display_entry.position();
      if total_size == 0 {
        (String::new(), 0)
      } else {
        let total_size_str = human_download_size(total_size, total_size);
        (
          format!(
            " {}/{}",
            human_download_size(pos, total_size),
            total_size_str,
          ),
          2 + total_size_str.len() * 2,
        )
      }
    };
    let (total_text, total_text_max_width) = if data.total_entries <= 1 {
      (String::new(), 0)
    } else {
      let total_entries_str = data.total_entries.to_string();
      (
        format!(
          " ({}/{})",
          data.total_entries - data.entries.len(),
          data.total_entries
        ),
        4 + total_entries_str.len() * 2,
      )
    };

    let elapsed_text = get_elapsed_text(data.duration);
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
    // todo: handle subtracting going below zero here
    let total_bars = (std::cmp::min(75, data.terminal_width - 5) as usize)
      - elapsed_text.len()
      - total_text_max_width
      - bytes_text_max_width
      - 3; // space, open and close brace
    let completed_bars = (total_bars as f64 * percent_done).floor() as usize;
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
}

/// Indicatif style progress bar.
#[derive(Debug)]
pub struct TextOnlyProgressBarRenderer;

impl ProgressBarRenderer for TextOnlyProgressBarRenderer {
  fn render(&self, data: ProgressData) -> String {
    let display_entry = match data.preferred_display_entry() {
      Some(v) => v,
      None => return String::new(),
    };

    let bytes_text = {
      let total_size = display_entry.total_size();
      let pos = display_entry.position();
      if total_size == 0 {
        String::new()
      } else {
        format!(
          " {}/{}",
          human_download_size(pos, total_size),
          human_download_size(total_size, total_size)
        )
      }
    };
    let total_text = if data.total_entries <= 1 {
      String::new()
    } else {
      format!(
        " ({}/{})",
        data.total_entries - data.entries.len(),
        data.total_entries
      )
    };

    format!(
      "{} {}{}{}",
      colors::green("Download"),
      display_entry.message,
      bytes_text,
      total_text,
    )
  }
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
