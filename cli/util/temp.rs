// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use chrono::NaiveDate;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
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

/// Creates a node_modules directory, normally in a folder with the following
/// format:
///
///   <tmp-dir>/deno_nm/<date>/<random-value>
///
/// Old dated folders are automatically deleted. If the dated parent cannot be
/// used, this falls back to a fresh directory directly beneath `<tmp-dir>`.
pub fn create_temp_node_modules_dir() -> Result<TempNodeModulesDir, AnyError> {
  let temp_folder = canonicalize_path(&std::env::temp_dir())
    .context("Failed resolving temporary directory")?;
  ensure_secure_temp_parent(&temp_folder)?;
  let today = chrono::Utc::now().date_naive();
  if let Err(err) =
    attempt_fallback_temp_dir_garbage_collection(&temp_folder, today)
  {
    log::debug!("Failed fallback temp folder garbage collection: {:#?}", err);
  }
  let root_temp_folder = temp_folder.join("deno_nm");
  let temp_node_modules_parent_dir =
    match create_secure_day_folder(&root_temp_folder, today) {
      Ok(day_folder) => tempfile::TempDir::new_in(&day_folder)?,
      Err(err) => {
        log::debug!(
          "Failed creating secure temp node_modules folder at '{}': {:#?}",
          root_temp_folder.display(),
          err
        );
        let prefix =
          format!("deno_nm_{}_", folder_name_for_date(today).to_string_lossy());
        tempfile::Builder::new()
          .prefix(&prefix)
          .tempdir_in(&temp_folder)
          .context("Failed creating temp node_modules folder")?
      }
    };
  // write a package.json to make this be considered a "node" project to deno
  let package_json_path =
    temp_node_modules_parent_dir.path().join("package.json");
  std::fs::write(&package_json_path, "{}").with_context(|| {
    format!("Failed creating '{}'", package_json_path.display())
  })?;
  let temp_dir_path = canonicalize_path(temp_node_modules_parent_dir.path())
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

fn create_secure_day_folder(
  root_temp_folder: &Path,
  today: NaiveDate,
) -> Result<PathBuf, AnyError> {
  create_secure_temp_dir(root_temp_folder).with_context(|| {
    format!("Failed creating '{}'", root_temp_folder.display())
  })?;

  // remove any old/stale temp dirs only after confirming the root is owned by
  // the current user and not writable by other users.
  if let Err(err) = attempt_temp_dir_garbage_collection(root_temp_folder, today)
  {
    log::debug!("Failed init temp folder garbage collection: {:#?}", err);
  }

  let day_folder = root_temp_folder.join(folder_name_for_date(today));
  create_secure_temp_dir(&day_folder)
    .with_context(|| format!("Failed creating '{}'", day_folder.display()))?;
  Ok(day_folder)
}

fn create_secure_temp_dir(path: &Path) -> Result<(), AnyError> {
  match create_dir_secure(path) {
    Ok(()) => {}
    Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
    Err(err) => return Err(err.into()),
  }
  ensure_secure_temp_dir(path)
}

#[cfg(unix)]
fn ensure_secure_temp_parent(path: &Path) -> Result<(), AnyError> {
  use std::os::unix::fs::MetadataExt;
  use std::os::unix::fs::OpenOptionsExt;

  // SAFETY: geteuid has no preconditions.
  let current_uid = unsafe { libc::geteuid() };
  for ancestor in path.ancestors() {
    let dir = std::fs::OpenOptions::new()
      .read(true)
      .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
      .open(ancestor)?;
    let metadata = dir.metadata()?;
    if metadata.uid() != current_uid && metadata.uid() != 0 {
      bail!(
        "temporary directory ancestor '{}' is owned by uid {}, not current uid {} or root",
        ancestor.display(),
        metadata.uid(),
        current_uid
      );
    }
    let mode = metadata.mode();
    if mode & 0o022 != 0 && mode & 0o1000 == 0 {
      bail!(
        "temporary directory ancestor '{}' is writable by other users without the sticky bit",
        ancestor.display()
      );
    }
  }
  Ok(())
}

#[cfg(not(unix))]
fn ensure_secure_temp_parent(path: &Path) -> Result<(), AnyError> {
  let metadata = std::fs::symlink_metadata(path)?;
  if metadata.file_type().is_symlink() || !metadata.is_dir() {
    bail!("'{}' is not a directory", path.display());
  }
  Ok(())
}

#[cfg(unix)]
fn create_dir_secure(path: &Path) -> std::io::Result<()> {
  use std::os::unix::fs::DirBuilderExt;

  std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_dir_secure(path: &Path) -> std::io::Result<()> {
  std::fs::create_dir(path)
}

#[cfg(unix)]
fn ensure_secure_temp_dir(path: &Path) -> Result<(), AnyError> {
  use std::os::unix::fs::MetadataExt;
  use std::os::unix::fs::OpenOptionsExt;
  use std::os::unix::fs::PermissionsExt;

  // Keep the check, permission repair, and final validation tied to the same
  // directory so replacing the path cannot redirect the chmod. The canonical
  // temp path and its ancestors are validated separately before subsequent
  // path-based use.
  let dir = std::fs::OpenOptions::new()
    .read(true)
    .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
    .open(path)?;

  // SAFETY: geteuid has no preconditions.
  let current_uid = unsafe { libc::geteuid() };
  let metadata = dir.metadata()?;
  if metadata.uid() != current_uid {
    bail!(
      "'{}' is owned by uid {}, not current uid {}",
      path.display(),
      metadata.uid(),
      current_uid
    );
  }

  let mode = metadata.permissions().mode();
  if mode & 0o077 != 0 {
    dir.set_permissions(std::fs::Permissions::from_mode(0o700))?;
  }

  let metadata = dir.metadata()?;
  if metadata.uid() != current_uid {
    bail!(
      "'{}' is owned by uid {}, not current uid {}",
      path.display(),
      metadata.uid(),
      current_uid
    );
  }
  if metadata.permissions().mode() & 0o077 != 0 {
    bail!(
      "'{}' is writable or readable by other users",
      path.display()
    );
  }

  Ok(())
}

#[cfg(not(unix))]
fn ensure_secure_temp_dir(path: &Path) -> Result<(), AnyError> {
  let metadata = std::fs::symlink_metadata(path)?;
  if metadata.file_type().is_symlink() || !metadata.is_dir() {
    bail!("'{}' is not a directory", path.display());
  }
  Ok(())
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

fn attempt_fallback_temp_dir_garbage_collection(
  temp_folder: &Path,
  utc_now: NaiveDate,
) -> Result<(), AnyError> {
  let previous_day = utc_now
    .checked_sub_days(chrono::Days::new(1))
    .unwrap_or(utc_now);
  let next_day = utc_now
    .checked_add_days(chrono::Days::new(1))
    .unwrap_or(utc_now);
  let progress_bar =
    ProgressBar::new(crate::util::progress_bar::ProgressBarStyle::TextOnly);
  let update_guard = progress_bar.deferred_update_with_prompt(
    crate::util::progress_bar::ProgressMessagePrompt::Cleaning,
    "old fallback temp node_modules folders...",
  );

  let mut cleaner = FsCleaner::new(Some(update_guard));
  for entry in std::fs::read_dir(temp_folder)? {
    let Ok(entry) = entry else {
      continue;
    };
    let file_name = entry.file_name();
    let Some(name) = file_name.to_str() else {
      continue;
    };
    let Some(suffix) = name.strip_prefix("deno_nm_") else {
      continue;
    };
    let Some((date, random)) = suffix.split_once('_') else {
      continue;
    };
    let Ok(date) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
      continue;
    };
    if random.is_empty()
      || date == previous_day
      || date == utc_now
      || date == next_day
    {
      continue;
    }

    let path = entry.path();
    if let Err(err) = ensure_secure_temp_dir(&path) {
      log::debug!(
        "Skipping cleanup of untrusted fallback temp folder '{}': {:#?}",
        path.display(),
        err
      );
      continue;
    }
    if let Err(err) = cleaner.rm_rf(&path) {
      log::debug!(
        "Failed cleaning fallback temp folder '{}': {:#?}",
        path.display(),
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

  #[test]
  fn test_attempt_fallback_temp_dir_garbage_collection() {
    let temp_dir = TempDir::new();
    let reference_date = chrono::NaiveDate::from_ymd_opt(2020, 5, 13).unwrap();
    temp_dir
      .path()
      .join("deno_nm_2020-05-01_old/node_modules")
      .create_dir_all();
    temp_dir
      .path()
      .join("deno_nm_2020-05-12_previous")
      .create_dir_all();
    temp_dir
      .path()
      .join("deno_nm_2020-05-13_current")
      .create_dir_all();
    temp_dir
      .path()
      .join("deno_nm_2020-05-14_next")
      .create_dir_all();
    temp_dir.path().join("deno_nm_legacy").create_dir_all();
    temp_dir.path().join("unrelated").create_dir_all();

    attempt_fallback_temp_dir_garbage_collection(
      temp_dir.path().as_path(),
      reference_date,
    )
    .unwrap();

    assert!(!temp_dir.path().join("deno_nm_2020-05-01_old").exists());
    assert!(temp_dir.path().join("deno_nm_2020-05-12_previous").exists());
    assert!(temp_dir.path().join("deno_nm_2020-05-13_current").exists());
    assert!(temp_dir.path().join("deno_nm_2020-05-14_next").exists());
    assert!(temp_dir.path().join("deno_nm_legacy").exists());
    assert!(temp_dir.path().join("unrelated").exists());
  }

  #[cfg(unix)]
  #[test]
  fn test_ensure_secure_temp_parent_rejects_non_sticky_writable_dir() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new();
    let child = temp_dir.path().join("private");
    child.create_dir_all();
    let child = canonicalize_path(child.as_path()).unwrap();
    std::fs::set_permissions(
      temp_dir.path(),
      std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(ensure_secure_temp_parent(&child).is_err());

    std::fs::set_permissions(
      temp_dir.path(),
      std::fs::Permissions::from_mode(0o1777),
    )
    .unwrap();
    ensure_secure_temp_parent(&child).unwrap();
  }

  #[cfg(unix)]
  #[test]
  fn test_create_secure_day_folder_repairs_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new();
    let root = temp_dir.path().join("deno_nm");
    let reference_date = chrono::NaiveDate::from_ymd_opt(2020, 5, 13).unwrap();
    root.create_dir_all();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))
      .unwrap();

    let day_folder =
      create_secure_day_folder(root.as_path(), reference_date).unwrap();
    assert_eq!(day_folder, root.join("2020-05-13").to_path_buf());
    assert_eq!(
      std::fs::metadata(&root).unwrap().permissions().mode() & 0o077,
      0
    );
    assert_eq!(
      std::fs::metadata(&day_folder).unwrap().permissions().mode() & 0o077,
      0
    );
  }

  #[cfg(unix)]
  #[test]
  fn test_create_secure_day_folder_rejects_symlink() {
    let temp_dir = TempDir::new();
    let target = temp_dir.path().join("target");
    let stale_folder = target.join("2020-05-01");
    stale_folder.create_dir_all();
    let sentinel = stale_folder.join("sentinel");
    sentinel.write("");
    let root = temp_dir.path().join("deno_nm");
    std::os::unix::fs::symlink(&target, &root).unwrap();
    let reference_date = chrono::NaiveDate::from_ymd_opt(2020, 5, 13).unwrap();

    assert!(create_secure_day_folder(root.as_path(), reference_date).is_err());
    assert!(sentinel.exists());
    assert!(!target.join("2020-05-13").exists());
  }
}
