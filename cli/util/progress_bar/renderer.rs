// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use deno_terminal::colors;

use crate::util::display::human_download_size;

use super::ProgressMessagePrompt;

#[derive(Clone)]
pub struct ProgressDataDisplayEntry {
  pub prompt: ProgressMessagePrompt,
  pub message: String,
  pub position: u64,
  pub total_size: u64,
}

#[derive(Clone)]
pub struct ProgressData {
  pub terminal_width: u32,
  pub display_entries: Vec<ProgressDataDisplayEntry>,
  pub pending_entries: usize,
  pub percent_done: f64,
  pub total_entries: usize,
  pub duration: Duration,
}

pub trait ProgressBarRenderer: Send + Sync + std::fmt::Debug {
  fn render(&self, data: ProgressData) -> String;
}

/// Indicatif style progress bar.
#[derive(Debug)]
pub struct BarProgressBarRenderer {
  pub display_human_download_size: bool,
}

impl ProgressBarRenderer for BarProgressBarRenderer {
  fn render(&self, data: ProgressData) -> String {
    // In `ProgressBarRenderer` we only care about first entry.
    let Some(display_entry) = &data.display_entries.first() else {
      return String::new();
    };
    let (bytes_text, bytes_text_max_width) = {
      let total_size = display_entry.total_size;
      let pos = display_entry.position;
      if total_size == 0 {
        (String::new(), 0)
      } else {
        let (pos_str, total_size_str) = if self.display_human_download_size {
          (
            human_download_size(pos, total_size),
            human_download_size(total_size, total_size),
          )
        } else {
          (pos.to_string(), total_size.to_string())
        };
        (
          format!(" {}/{}", pos_str, total_size_str,),
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
          data.total_entries - data.pending_entries,
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
    let max_width = (data.terminal_width as i32 - 5).clamp(10, 75) as usize;
    let same_line_text_width =
      elapsed_text.len() + total_text_max_width + bytes_text_max_width + 3; // space, open and close brace
    let total_bars = if same_line_text_width > max_width {
      1
    } else {
      max_width - same_line_text_width
    };
    let completed_bars =
      (total_bars as f64 * data.percent_done).floor() as usize;
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
      text.push_str(&colors::gray(bytes_text).to_string());
    }
    text.push_str(&colors::gray(total_text).to_string());

    text
  }
}

#[derive(Debug)]
pub struct TextOnlyProgressBarRenderer {
  last_tick: AtomicUsize,
  start_time: std::time::Instant,
}

impl Default for TextOnlyProgressBarRenderer {
  fn default() -> Self {
    Self {
      last_tick: Default::default(),
      start_time: std::time::Instant::now(),
    }
  }
}

const SPINNER_CHARS: [&str; 8] = ["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];
impl ProgressBarRenderer for TextOnlyProgressBarRenderer {
  fn render(&self, data: ProgressData) -> String {
    let last_tick = {
      let last_tick = self.last_tick.load(Ordering::Relaxed);
      let last_tick = (last_tick + 1) % 8;
      self.last_tick.store(last_tick, Ordering::Relaxed);
      last_tick
    };
    let current_time = std::time::Instant::now();

    let mut display_str = format!(
      "{} {} ",
      data.display_entries[0].prompt.as_text(),
      SPINNER_CHARS[last_tick]
    );

    let elapsed_time = current_time - self.start_time;
    let fmt_elapsed_time = get_elapsed_text(elapsed_time);

    let total_text = if data.total_entries <= 1 {
      String::new()
    } else {
      format!(
        " {}/{}",
        data.total_entries - data.pending_entries,
        data.total_entries
      )
    };

    display_str.push_str(&format!("{}{}\n", fmt_elapsed_time, total_text));

    for i in 0..4 {
      let Some(display_entry) = data.display_entries.get(i) else {
        display_str.push('\n');
        continue;
      };

      let bytes_text = {
        let total_size = display_entry.total_size;
        let pos = display_entry.position;
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

      let message = display_entry
        .message
        .replace("https://registry.npmjs.org/", "npm:")
        .replace("https://jsr.io/", "jsr:");
      display_str.push_str(
        &colors::gray(format!(" - {}{}\n", message, bytes_text)).to_string(),
      );
    }

    display_str
  }
}

fn get_elapsed_text(elapsed: Duration) -> String {
  let elapsed_secs = elapsed.as_secs();
  let seconds = elapsed_secs % 60;
  let minutes = elapsed_secs / 60;
  format!("[{minutes:0>2}:{seconds:0>2}]")
}

#[cfg(test)]
mod test {
  use super::*;
  use pretty_assertions::assert_eq;
  use std::time::Duration;
  use test_util::assert_contains;

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
    assert_eq!(get_elapsed_text(Duration::from_secs(60 * 60)), "[60:00]");
    assert_eq!(
      get_elapsed_text(Duration::from_secs(60 * 60 * 3 + 20 * 60 + 2)),
      "[200:02]"
    );
    assert_eq!(
      get_elapsed_text(Duration::from_secs(60 * 60 * 99)),
      "[5940:00]"
    );
  }

  const BYTES_TO_KIB: u64 = 2u64.pow(10);

  #[test]
  fn should_render_bar_progress() {
    let renderer = BarProgressBarRenderer {
      display_human_download_size: true,
    };
    let mut data = ProgressData {
      display_entries: vec![ProgressDataDisplayEntry {
        prompt: ProgressMessagePrompt::Download,
        message: "data".to_string(),
        position: 0,
        total_size: 10 * BYTES_TO_KIB,
      }],
      duration: Duration::from_secs(1),
      pending_entries: 1,
      total_entries: 1,
      percent_done: 0f64,
      terminal_width: 50,
    };
    let text = renderer.render(data.clone());
    let text = test_util::strip_ansi_codes(&text);
    assert_eq!(
      text,
      concat!(
        "Download data 0.00KiB/10.00KiB\n",
        "[00:01] [-----------------]",
      ),
    );

    data.percent_done = 0.5f64;
    data.display_entries[0].position = 5 * BYTES_TO_KIB;
    data.display_entries[0].message = "".to_string();
    data.total_entries = 3;
    let text = renderer.render(data.clone());
    let text = test_util::strip_ansi_codes(&text);
    assert_eq!(text, "[00:01] [####>------] 5.00KiB/10.00KiB (2/3)",);

    // just ensure this doesn't panic
    data.terminal_width = 0;
    let text = renderer.render(data.clone());
    let text = test_util::strip_ansi_codes(&text);
    assert_eq!(text, "[00:01] [-] 5.00KiB/10.00KiB (2/3)",);

    data.terminal_width = 50;
    data.pending_entries = 0;
    data.display_entries[0].position = 10 * BYTES_TO_KIB;
    data.percent_done = 1.0f64;
    let text = renderer.render(data.clone());
    let text = test_util::strip_ansi_codes(&text);
    assert_eq!(text, "[00:01] [###########] 10.00KiB/10.00KiB (3/3)",);

    data.display_entries[0].position = 0;
    data.display_entries[0].total_size = 0;
    data.pending_entries = 0;
    data.total_entries = 1;
    let text = renderer.render(data);
    let text = test_util::strip_ansi_codes(&text);
    assert_eq!(text, "[00:01] [###################################]",);
  }

  #[test]
  fn should_render_text_only_progress() {
    let renderer = TextOnlyProgressBarRenderer::default();
    let mut data = ProgressData {
      display_entries: vec![ProgressDataDisplayEntry {
        prompt: ProgressMessagePrompt::Blocking,
        message: "data".to_string(),
        position: 0,
        total_size: 10 * BYTES_TO_KIB,
      }],
      duration: Duration::from_secs(1),
      pending_entries: 1,
      total_entries: 3,
      percent_done: 0f64,
      terminal_width: 50,
    };
    let text = renderer.render(data.clone());
    let text = test_util::strip_ansi_codes(&text);
    assert_contains!(text, "Blocking ⣯");
    assert_contains!(text, "2/3\n - data 0.00KiB/10.00KiB\n\n\n\n");

    data.pending_entries = 0;
    data.total_entries = 1;
    data.display_entries[0].position = 0;
    data.display_entries[0].total_size = 0;
    let text = renderer.render(data);
    let text = test_util::strip_ansi_codes(&text);
    assert_contains!(text, "Blocking ⣟");
    assert_contains!(text, "\n - data\n\n\n\n");
  }
}
