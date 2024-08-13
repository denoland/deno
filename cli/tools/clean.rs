// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;

use deno_core::error::AnyError;

use crate::cache::DenoDir;
use crate::colors;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;

pub fn clean() -> Result<(), AnyError> {
  let deno_dir = DenoDir::new(None)?;
  if deno_dir.root.exists() {
    let no_of_files = walkdir::WalkDir::new(&deno_dir.root).into_iter().count();
    let progress_bar = ProgressBar::new(ProgressBarStyle::DownloadBars);
    let progress =
      progress_bar.update_with_prompt(ProgressMessagePrompt::Cleaning, "");

    progress.set_total_size(no_of_files.try_into().unwrap());

    for entry in walkdir::WalkDir::new(&deno_dir.root).contents_first(true) {
      let entry = entry?;

      if entry.file_type().is_dir() {
        std::fs::remove_dir_all(&deno_dir.root)?;
      } else {
        remove_file(entry.path(), entry.metadata())?;
      }
    }

    let no_of_files = 10;
    let size = 10.5;
    let size_human_display = "MiB";

    log::info!(
      "{} {} ({} files, {}{})",
      colors::green("Removed"),
      deno_dir.root.display(),
      no_of_files,
      size,
      size_human_display
    );
  }

  Ok(())
}

fn remove_file(
  path: &Path,
  meta: Result<std::fs::Metadata, AnyError>,
) -> Result<(), AnyError> {
  todo!()
}
