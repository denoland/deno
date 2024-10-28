// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::path::Path;

use crate::cache::DenoDir;
use crate::colors;
use crate::display;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;
use crate::util::progress_bar::UpdateGuard;

struct CleanState {
  files_removed: u64,
  dirs_removed: u64,
  bytes_removed: u64,
  progress_guard: UpdateGuard,
}

impl CleanState {
  fn update_progress(&self) {
    self
      .progress_guard
      .set_position(self.files_removed + self.dirs_removed);
  }
}

pub fn clean() -> Result<(), AnyError> {
  let deno_dir = DenoDir::new(None)?;
  if deno_dir.root.exists() {
    let no_of_files = walkdir::WalkDir::new(&deno_dir.root).into_iter().count();
    let progress_bar = ProgressBar::new(ProgressBarStyle::ProgressBars);
    let progress_guard =
      progress_bar.update_with_prompt(ProgressMessagePrompt::Cleaning, "");

    let mut state = CleanState {
      files_removed: 0,
      dirs_removed: 0,
      bytes_removed: 0,
      progress_guard,
    };
    state
      .progress_guard
      .set_total_size(no_of_files.try_into().unwrap());

    rm_rf(&mut state, &deno_dir.root)?;

    // Drop the guard so that progress bar disappears.
    drop(state.progress_guard);

    log::info!(
      "{} {} {}",
      colors::green("Removed"),
      deno_dir.root.display(),
      colors::gray(&format!(
        "({} files, {})",
        state.files_removed + state.dirs_removed,
        display::human_size(state.bytes_removed as f64)
      ))
    );
  }

  Ok(())
}

fn rm_rf(state: &mut CleanState, path: &Path) -> Result<(), AnyError> {
  for entry in walkdir::WalkDir::new(path).contents_first(true) {
    let entry = entry?;

    if entry.file_type().is_dir() {
      state.dirs_removed += 1;
      state.update_progress();
      std::fs::remove_dir_all(entry.path())?;
    } else {
      remove_file(state, entry.path(), entry.metadata().ok())?;
    }
  }

  Ok(())
}

fn remove_file(
  state: &mut CleanState,
  path: &Path,
  meta: Option<std::fs::Metadata>,
) -> Result<(), AnyError> {
  if let Some(meta) = meta {
    state.bytes_removed += meta.len();
  }
  state.files_removed += 1;
  state.update_progress();
  std::fs::remove_file(path)
    .with_context(|| format!("Failed to remove file: {}", path.display()))?;
  Ok(())
}
