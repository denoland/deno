// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use chrono::NaiveDate;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;

use super::fs::FsCleaner;
use super::fs::canonicalize_path;
use super::progress_bar::ProgressBar;

pub struct TempNodeModulesDir {
  node_modules_dir_path: PathBuf,
  // keep alive
  _temp_dir: tempfile::TempDir,
}

impl TempNodeModulesDir {
  pub fn parent(&self) -> &Path {
    self.node_modules_dir_path.parent().unwrap()
  }

  pub fn node_modules_dir_path(&self) -> &Path {
    &self.node_modules_dir_path
  }
}

/// Creates a node_modules directory in a folder with the following format:
///
///   <tmp-dir>/deno_nm/<date>/<random-value>
///
/// Old folders are automatically deleted by this function.
pub fn create_temp_node_modules_dir() -> Result<TempNodeModulesDir, AnyError> {
  let root_temp_folder = std::env::temp_dir().join("deno_nm");
  let today = chrono::Utc::now().date_naive();
  // remove any old/stale temp dirs
  if let Err(err) =
    attempt_temp_dir_garbage_collection(&root_temp_folder, today)
  {
    log::debug!("Failed init temp folder garbage collection: {:#?}", err);
  }
  let day_folder = root_temp_folder.join(folder_name_for_date(today));
  std::fs::create_dir_all(&day_folder)
    .with_context(|| format!("Failed creating '{}'", day_folder.display()))?;
  let temp_node_modules_parent_dir = tempfile::TempDir::new_in(&day_folder)?;
  // write a package.json to make this be considered a "node" project to deno
  let package_json_path =
    temp_node_modules_parent_dir.path().join("package.json");
  std::fs::write(&package_json_path, "{}").with_context(|| {
    format!("Failed creating '{}'", package_json_path.display())
  })?;
  let temp_dir_path =
    canonicalize_path(temp_node_modules_parent_dir.path())
      .unwrap_or_else(|_| temp_node_modules_parent_dir.path().to_path_buf());
  let node_modules_dir_path = temp_dir_path.join("node_modules");
  log::debug!(
    "Creating node_modules directory at: {}",
    node_modules_dir_path.display()
  );
  Ok(TempNodeModulesDir {
    node_modules_dir_path,
    _temp_dir: temp_node_modules_parent_dir,
  })
}

fn attempt_temp_dir_garbage_collection(
  root_temp_folder: &Path,
  utc_now: NaiveDate,
) -> Result<(), AnyError> {
  let previous_day_str = folder_name_for_date(
    utc_now
      .checked_sub_days(chrono::Days::new(1))
      .unwrap_or(utc_now),
  );
  let current_day_str = folder_name_for_date(utc_now);
  let next_day_str = folder_name_for_date(
    utc_now
      .checked_add_days(chrono::Days::new(1))
      .unwrap_or(utc_now),
  );
  let progress_bar =
    ProgressBar::new(crate::util::progress_bar::ProgressBarStyle::TextOnly);
  let update_guard = progress_bar.deferred_update_with_prompt(
    crate::util::progress_bar::ProgressMessagePrompt::Cleaning,
    "old temp node_modules folders...",
  );

  // remove any folders that aren't the current date +- 1 day
  let mut cleaner = FsCleaner::new(Some(update_guard));
  for entry in std::fs::read_dir(root_temp_folder)? {
    let Ok(entry) = entry else {
      continue;
    };
    if entry.file_name() != previous_day_str
      && entry.file_name() != current_day_str
      && entry.file_name() != next_day_str
      && let Err(err) = cleaner.rm_rf(&entry.path())
    {
      log::debug!(
        "Failed cleaning '{}': {:#?}",
        entry.file_name().display(),
        err
      );
    }
  }

  Ok(())
}

fn folder_name_for_date(date: chrono::NaiveDate) -> OsString {
  OsString::from(date.format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_attempt_temp_dir_garbage_collection() {
    let temp_dir = TempDir::new();
    let reference_date = chrono::NaiveDate::from_ymd_opt(2020, 5, 13).unwrap();
    temp_dir.path().join("0000-00-00").create_dir_all();
    temp_dir
      .path()
      .join("2020-05-01/sub_dir/sub")
      .create_dir_all();
    temp_dir
      .path()
      .join("2020-05-01/sub_dir/sub/test.txt")
      .write("");
    temp_dir.path().join("2020-05-02/sub_dir").create_dir_all();
    temp_dir.path().join("2020-05-11").create_dir_all();
    temp_dir.path().join("2020-05-12").create_dir_all();
    temp_dir.path().join("2020-05-13").create_dir_all();
    temp_dir.path().join("2020-05-14").create_dir_all();
    temp_dir.path().join("2020-05-15").create_dir_all();
    attempt_temp_dir_garbage_collection(
      temp_dir.path().as_path(),
      reference_date,
    )
    .unwrap();
    let mut entries = std::fs::read_dir(temp_dir.path())
      .unwrap()
      .map(|e| e.unwrap().file_name().into_string().unwrap())
      .collect::<Vec<_>>();
    entries.sort();
    // should only have the current day +- 1
    assert_eq!(
      entries,
      vec![
        "2020-05-12".to_string(),
        "2020-05-13".to_string(),
        "2020-05-14".to_string()
      ]
    );
  }
}
