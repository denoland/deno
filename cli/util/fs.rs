// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
pub use deno_core::normalize_path;
use deno_core::unsync::spawn_blocking;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_crypto::rand;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::PathClean;
use std::borrow::Cow;
use std::env::current_dir;
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;

use crate::args::FilesConfig;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;

use super::path::specifier_to_file_path;

/// Writes the file to the file system at a temporary path, then
/// renames it to the destination in a single sys call in order
/// to never leave the file system in a corrupted state.
///
/// This also handles creating the directory if a NotFound error
/// occurs.
pub fn atomic_write_file<T: AsRef<[u8]>>(
  file_path: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  fn atomic_write_file_raw(
    temp_file_path: &Path,
    file_path: &Path,
    data: &[u8],
    mode: u32,
  ) -> std::io::Result<()> {
    write_file(temp_file_path, data, mode)?;
    std::fs::rename(temp_file_path, file_path)?;
    Ok(())
  }

  fn inner(file_path: &Path, data: &[u8], mode: u32) -> std::io::Result<()> {
    let temp_file_path = {
      let rand: String = (0..4).fold(String::new(), |mut output, _| {
        let _ = write!(output, "{:02x}", rand::random::<u8>());
        output
      });
      let extension = format!("{rand}.tmp");
      file_path.with_extension(extension)
    };

    if let Err(write_err) =
      atomic_write_file_raw(&temp_file_path, file_path, data, mode)
    {
      if write_err.kind() == ErrorKind::NotFound {
        let parent_dir_path = file_path.parent().unwrap();
        match std::fs::create_dir_all(parent_dir_path) {
          Ok(()) => {
            return atomic_write_file_raw(
              &temp_file_path,
              file_path,
              data,
              mode,
            )
            .map_err(|err| add_file_context_to_err(file_path, err));
          }
          Err(create_err) => {
            if !parent_dir_path.exists() {
              return Err(Error::new(
                create_err.kind(),
                format!(
                  "{:#} (for '{}')\nCheck the permission of the directory.",
                  create_err,
                  parent_dir_path.display()
                ),
              ));
            }
          }
        }
      }
      return Err(add_file_context_to_err(file_path, write_err));
    }
    Ok(())
  }

  inner(file_path, data.as_ref(), mode)
}

/// Creates a std::fs::File handling if the parent does not exist.
pub fn create_file(file_path: &Path) -> std::io::Result<std::fs::File> {
  match std::fs::File::create(file_path) {
    Ok(file) => Ok(file),
    Err(err) => {
      if err.kind() == ErrorKind::NotFound {
        let parent_dir_path = file_path.parent().unwrap();
        match std::fs::create_dir_all(parent_dir_path) {
          Ok(()) => {
            return std::fs::File::create(file_path)
              .map_err(|err| add_file_context_to_err(file_path, err));
          }
          Err(create_err) => {
            if !parent_dir_path.exists() {
              return Err(Error::new(
                create_err.kind(),
                format!(
                  "{:#} (for '{}')\nCheck the permission of the directory.",
                  create_err,
                  parent_dir_path.display()
                ),
              ));
            }
          }
        }
      }
      Err(add_file_context_to_err(file_path, err))
    }
  }
}

fn add_file_context_to_err(file_path: &Path, err: Error) -> Error {
  Error::new(
    err.kind(),
    format!("{:#} (for '{}')", err, file_path.display()),
  )
}

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  write_file_2(filename, data, true, mode, true, false)
}

pub fn write_file_2<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  update_mode: bool,
  mode: u32,
  is_create: bool,
  is_append: bool,
) -> std::io::Result<()> {
  let mut file = OpenOptions::new()
    .read(false)
    .write(true)
    .append(is_append)
    .truncate(!is_append)
    .create(is_create)
    .open(filename)?;

  if update_mode {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = mode & 0o777;
      let permissions = PermissionsExt::from_mode(mode);
      file.set_permissions(permissions)?;
    }
    #[cfg(not(unix))]
    let _ = mode;
  }

  file.write_all(data.as_ref())
}

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  Ok(deno_core::strip_unc_prefix(path.canonicalize()?))
}

/// Canonicalizes a path which might be non-existent by going up the
/// ancestors until it finds a directory that exists, canonicalizes
/// that path, then adds back the remaining path components.
///
/// Note: When using this, you should be aware that a symlink may
/// subsequently be created along this path by some other code.
pub fn canonicalize_path_maybe_not_exists(
  path: &Path,
) -> Result<PathBuf, Error> {
  canonicalize_path_maybe_not_exists_with_custom_fn(path, canonicalize_path)
}

pub fn canonicalize_path_maybe_not_exists_with_fs(
  path: &Path,
  fs: &dyn FileSystem,
) -> Result<PathBuf, Error> {
  canonicalize_path_maybe_not_exists_with_custom_fn(path, |path| {
    fs.realpath_sync(path).map_err(|err| err.into_io_error())
  })
}

fn canonicalize_path_maybe_not_exists_with_custom_fn(
  path: &Path,
  canonicalize: impl Fn(&Path) -> Result<PathBuf, Error>,
) -> Result<PathBuf, Error> {
  let path = path.to_path_buf().clean();
  let mut path = path.as_path();
  let mut names_stack = Vec::new();
  loop {
    match canonicalize(path) {
      Ok(mut canonicalized_path) => {
        for name in names_stack.into_iter().rev() {
          canonicalized_path = canonicalized_path.join(name);
        }
        return Ok(canonicalized_path);
      }
      Err(err) if err.kind() == ErrorKind::NotFound => {
        names_stack.push(match path.file_name() {
          Some(name) => name.to_owned(),
          None => return Err(err),
        });
        path = match path.parent() {
          Some(parent) => parent,
          None => return Err(err),
        };
      }
      Err(err) => return Err(err),
    }
  }
}

pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  let resolved_path = if path.is_absolute() {
    path.to_owned()
  } else {
    let cwd =
      current_dir().context("Failed to get current working directory")?;
    cwd.join(path)
  };

  Ok(normalize_path(resolved_path))
}

/// Collects file paths that satisfy the given predicate, by recursively walking `files`.
/// If the walker visits a path that is listed in `ignore`, it skips descending into the directory.
pub struct FileCollector<TFilter: Fn(&Path) -> bool> {
  canonicalized_ignore: Vec<PathBuf>,
  file_filter: TFilter,
  ignore_git_folder: bool,
  ignore_node_modules: bool,
  ignore_vendor_folder: bool,
}

impl<TFilter: Fn(&Path) -> bool> FileCollector<TFilter> {
  pub fn new(file_filter: TFilter) -> Self {
    Self {
      canonicalized_ignore: Default::default(),
      file_filter,
      ignore_git_folder: false,
      ignore_node_modules: false,
      ignore_vendor_folder: false,
    }
  }

  pub fn add_ignore_paths(mut self, paths: &[PathBuf]) -> Self {
    // retain only the paths which exist and ignore the rest
    self
      .canonicalized_ignore
      .extend(paths.iter().filter_map(|i| canonicalize_path(i).ok()));
    self
  }

  pub fn ignore_node_modules(mut self) -> Self {
    self.ignore_node_modules = true;
    self
  }

  pub fn ignore_vendor_folder(mut self) -> Self {
    self.ignore_vendor_folder = true;
    self
  }

  pub fn ignore_git_folder(mut self) -> Self {
    self.ignore_git_folder = true;
    self
  }

  pub fn collect_files(
    &self,
    files: Option<&[PathBuf]>,
  ) -> Result<Vec<PathBuf>, AnyError> {
    let mut target_files = Vec::new();
    let files = if let Some(files) = files {
      Cow::Borrowed(files)
    } else {
      Cow::Owned(vec![PathBuf::from(".")])
    };
    for file in files.iter() {
      if let Ok(file) = canonicalize_path(file) {
        // use an iterator like this in order to minimize the number of file system operations
        let mut iterator = WalkDir::new(&file).into_iter();
        loop {
          let e = match iterator.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(entry)) => entry,
          };
          let file_type = e.file_type();
          let is_dir = file_type.is_dir();
          if let Ok(c) = canonicalize_path(e.path()) {
            if self.canonicalized_ignore.iter().any(|i| c.starts_with(i)) {
              if is_dir {
                iterator.skip_current_dir();
              }
            } else if is_dir {
              let should_ignore_dir = c
                .file_name()
                .map(|dir_name| {
                  let dir_name = dir_name.to_string_lossy().to_lowercase();
                  let is_ignored_file = match dir_name.as_str() {
                    "node_modules" => self.ignore_node_modules,
                    "vendor" => self.ignore_vendor_folder,
                    ".git" => self.ignore_git_folder,
                    _ => false,
                  };
                  // allow the user to opt out of ignoring by explicitly specifying the dir
                  file != c && is_ignored_file
                })
                .unwrap_or(false);
              if should_ignore_dir {
                iterator.skip_current_dir();
              }
            } else if (self.file_filter)(e.path()) {
              target_files.push(c);
            }
          } else if is_dir {
            // failed canonicalizing, so skip it
            iterator.skip_current_dir();
          }
        }
      }
    }
    Ok(target_files)
  }
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
/// Note: This ignores all .git and node_modules folders.
pub fn collect_specifiers(
  files: &FilesConfig,
  predicate: impl Fn(&Path) -> bool,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let mut prepared = vec![];
  let file_collector = FileCollector::new(predicate)
    .add_ignore_paths(&files.exclude)
    .ignore_git_folder()
    .ignore_node_modules()
    .ignore_vendor_folder();

  let root_path = current_dir()?;
  let include_files = if let Some(include) = &files.include {
    Cow::Borrowed(include)
  } else {
    Cow::Owned(vec![root_path.clone()])
  };
  for path in include_files.iter() {
    let path = path.to_string_lossy();
    let lowercase_path = path.to_lowercase();
    if lowercase_path.starts_with("http://")
      || lowercase_path.starts_with("https://")
    {
      let url = ModuleSpecifier::parse(&path)?;
      prepared.push(url);
      continue;
    }

    let p = if lowercase_path.starts_with("file://") {
      specifier_to_file_path(&ModuleSpecifier::parse(&path)?)?
    } else {
      root_path.join(path.as_ref())
    };
    let p = normalize_path(p);
    if p.is_dir() {
      let test_files = file_collector.collect_files(Some(&[p]))?;
      let mut test_files_as_urls = test_files
        .iter()
        .map(|f| ModuleSpecifier::from_file_path(f).unwrap())
        .collect::<Vec<ModuleSpecifier>>();

      test_files_as_urls.sort();
      prepared.extend(test_files_as_urls);
    } else {
      let url = ModuleSpecifier::from_file_path(p).unwrap();
      prepared.push(url);
    }
  }

  Ok(prepared)
}

/// Asynchronously removes a directory and all its descendants, but does not error
/// when the directory does not exist.
pub async fn remove_dir_all_if_exists(path: &Path) -> std::io::Result<()> {
  let result = tokio::fs::remove_dir_all(path).await;
  match result {
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    _ => result,
  }
}

/// Copies a directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(to)
    .with_context(|| format!("Creating {}", to.display()))?;
  let read_dir = std::fs::read_dir(from)
    .with_context(|| format!("Reading {}", from.display()))?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      copy_dir_recursive(&new_from, &new_to).with_context(|| {
        format!("Dir {} to {}", new_from.display(), new_to.display())
      })?;
    } else if file_type.is_file() {
      std::fs::copy(&new_from, &new_to).with_context(|| {
        format!("Copying {} to {}", new_from.display(), new_to.display())
      })?;
    }
  }

  Ok(())
}

/// Hardlinks the files in one directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn hard_link_dir_recursive(from: &Path, to: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(to)
    .with_context(|| format!("Creating {}", to.display()))?;
  let read_dir = std::fs::read_dir(from)
    .with_context(|| format!("Reading {}", from.display()))?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      hard_link_dir_recursive(&new_from, &new_to).with_context(|| {
        format!("Dir {} to {}", new_from.display(), new_to.display())
      })?;
    } else if file_type.is_file() {
      // note: chance for race conditions here between attempting to create,
      // then removing, then attempting to create. There doesn't seem to be
      // a way to hard link with overwriting in Rust, but maybe there is some
      // way with platform specific code. The workaround here is to handle
      // scenarios where something else might create or remove files.
      if let Err(err) = std::fs::hard_link(&new_from, &new_to) {
        if err.kind() == ErrorKind::AlreadyExists {
          if let Err(err) = std::fs::remove_file(&new_to) {
            if err.kind() == ErrorKind::NotFound {
              // Assume another process/thread created this hard link to the file we are wanting
              // to remove then sleep a little bit to let the other process/thread move ahead
              // faster to reduce contention.
              std::thread::sleep(Duration::from_millis(10));
            } else {
              return Err(err).with_context(|| {
                format!(
                  "Removing file to hard link {} to {}",
                  new_from.display(),
                  new_to.display()
                )
              });
            }
          }

          // Always attempt to recreate the hardlink. In contention scenarios, the other process
          // might have been killed or exited after removing the file, but before creating the hardlink
          if let Err(err) = std::fs::hard_link(&new_from, &new_to) {
            // Assume another process/thread created this hard link to the file we are wanting
            // to now create then sleep a little bit to let the other process/thread move ahead
            // faster to reduce contention.
            if err.kind() == ErrorKind::AlreadyExists {
              std::thread::sleep(Duration::from_millis(10));
            } else {
              return Err(err).with_context(|| {
                format!(
                  "Hard linking {} to {}",
                  new_from.display(),
                  new_to.display()
                )
              });
            }
          }
        } else {
          return Err(err).with_context(|| {
            format!(
              "Hard linking {} to {}",
              new_from.display(),
              new_to.display()
            )
          });
        }
      }
    }
  }

  Ok(())
}

pub fn symlink_dir(oldpath: &Path, newpath: &Path) -> Result<(), AnyError> {
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::symlink;
    symlink(oldpath, newpath).map_err(err_mapper)?;
  }
  #[cfg(not(unix))]
  {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(oldpath, newpath).map_err(err_mapper)?;
  }
  Ok(())
}

/// Gets the total size (in bytes) of a directory.
pub fn dir_size(path: &Path) -> std::io::Result<u64> {
  let entries = std::fs::read_dir(path)?;
  let mut total = 0;
  for entry in entries {
    let entry = entry?;
    total += match entry.metadata()? {
      data if data.is_dir() => dir_size(&entry.path())?,
      data => data.len(),
    };
  }
  Ok(total)
}

struct LaxSingleProcessFsFlagInner {
  file_path: PathBuf,
  fs_file: std::fs::File,
  finished_token: Arc<tokio_util::sync::CancellationToken>,
}

impl Drop for LaxSingleProcessFsFlagInner {
  fn drop(&mut self) {
    use fs3::FileExt;
    // kill the poll thread
    self.finished_token.cancel();
    // release the file lock
    if let Err(err) = self.fs_file.unlock() {
      log::debug!(
        "Failed releasing lock for {}. {:#}",
        self.file_path.display(),
        err
      );
    }
  }
}

/// A file system based flag that will attempt to synchronize multiple
/// processes so they go one after the other. In scenarios where
/// synchronization cannot be achieved, it will allow the current process
/// to proceed.
///
/// This should only be used in places where it's ideal for multiple
/// processes to not update something on the file system at the same time,
/// but it's not that big of a deal.
pub struct LaxSingleProcessFsFlag(Option<LaxSingleProcessFsFlagInner>);

impl LaxSingleProcessFsFlag {
  pub async fn lock(file_path: PathBuf, long_wait_message: &str) -> Self {
    log::debug!("Acquiring file lock at {}", file_path.display());
    use fs3::FileExt;
    let last_updated_path = file_path.with_extension("lock.poll");
    let start_instant = std::time::Instant::now();
    let open_result = std::fs::OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .open(&file_path);

    match open_result {
      Ok(fs_file) => {
        let mut pb_update_guard = None;
        let mut error_count = 0;
        while error_count < 10 {
          let lock_result = fs_file.try_lock_exclusive();
          let poll_file_update_ms = 100;
          match lock_result {
            Ok(_) => {
              log::debug!("Acquired file lock at {}", file_path.display());
              let _ignore = std::fs::write(&last_updated_path, "");
              let token = Arc::new(tokio_util::sync::CancellationToken::new());

              // Spawn a blocking task that will continually update a file
              // signalling the lock is alive. This is a fail safe for when
              // a file lock is never released. For example, on some operating
              // systems, if a process does not release the lock (say it's
              // killed), then the OS may release it at an indeterminate time
              //
              // This uses a blocking task because we use a single threaded
              // runtime and this is time sensitive so we don't want it to update
              // at the whims of of whatever is occurring on the runtime thread.
              spawn_blocking({
                let token = token.clone();
                let last_updated_path = last_updated_path.clone();
                move || {
                  let mut i = 0;
                  while !token.is_cancelled() {
                    i += 1;
                    let _ignore =
                      std::fs::write(&last_updated_path, i.to_string());
                    std::thread::sleep(Duration::from_millis(
                      poll_file_update_ms,
                    ));
                  }
                }
              });

              return Self(Some(LaxSingleProcessFsFlagInner {
                file_path,
                fs_file,
                finished_token: token,
              }));
            }
            Err(_) => {
              // show a message if it's been a while
              if pb_update_guard.is_none()
                && start_instant.elapsed().as_millis() > 1_000
              {
                let pb = ProgressBar::new(ProgressBarStyle::TextOnly);
                let guard = pb.update_with_prompt(
                  ProgressMessagePrompt::Blocking,
                  long_wait_message,
                );
                pb_update_guard = Some((guard, pb));
              }

              // sleep for a little bit
              tokio::time::sleep(Duration::from_millis(20)).await;

              // Poll the last updated path to check if it's stopped updating,
              // which is an indication that the file lock is claimed, but
              // was never properly released.
              match std::fs::metadata(&last_updated_path)
                .and_then(|p| p.modified())
              {
                Ok(last_updated_time) => {
                  let current_time = std::time::SystemTime::now();
                  match current_time.duration_since(last_updated_time) {
                    Ok(duration) => {
                      if duration.as_millis()
                        > (poll_file_update_ms * 2) as u128
                      {
                        // the other process hasn't updated this file in a long time
                        // so maybe it was killed and the operating system hasn't
                        // released the file lock yet
                        return Self(None);
                      } else {
                        error_count = 0; // reset
                      }
                    }
                    Err(_) => {
                      error_count += 1;
                    }
                  }
                }
                Err(_) => {
                  error_count += 1;
                }
              }
            }
          }
        }

        drop(pb_update_guard); // explicit for clarity
        Self(None)
      }
      Err(err) => {
        log::debug!(
          "Failed to open file lock at {}. {:#}",
          file_path.display(),
          err
        );
        Self(None) // let the process through
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::futures;
  use deno_core::parking_lot::Mutex;
  use pretty_assertions::assert_eq;
  use test_util::PathRef;
  use test_util::TempDir;
  use tokio::sync::Notify;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a/..")).unwrap(), cwd);
  }

  #[test]
  fn test_normalize_path() {
    assert_eq!(normalize_path(Path::new("a/../b")), PathBuf::from("b"));
    assert_eq!(normalize_path(Path::new("a/./b/")), PathBuf::from("a/b/"));
    assert_eq!(
      normalize_path(Path::new("a/./b/../c")),
      PathBuf::from("a/c")
    );

    if cfg!(windows) {
      assert_eq!(
        normalize_path(Path::new("C:\\a\\.\\b\\..\\c")),
        PathBuf::from("C:\\a\\c")
      );
    }
  }

  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("a");
    let cwd = current_dir().unwrap();
    let absolute_expected = cwd.join(expected);
    assert_eq!(resolve_from_cwd(expected).unwrap(), absolute_expected);
  }

  #[test]
  fn test_collect_files() {
    fn create_files(dir_path: &PathRef, files: &[&str]) {
      dir_path.create_dir_all();
      for f in files {
        dir_path.join(f).write("");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // |   ├── git
    // |   |   └── git.js
    // |   ├── node_modules
    // |   |   └── node_modules.js
    // |   ├── vendor
    // |   |   └── vendor.js
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    t.create_dir_all("dir.ts/child/node_modules");
    t.write("dir.ts/child/node_modules/node_modules.js", "");
    t.create_dir_all("dir.ts/child/.git");
    t.write("dir.ts/child/.git/git.js", "");
    t.create_dir_all("dir.ts/child/vendor");
    t.write("dir.ts/child/vendor/vendor.js", "");

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let file_collector = FileCollector::new(|path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|f| !f.starts_with('.'))
        .unwrap_or(false)
    })
    .add_ignore_paths(&[ignore_dir_path.to_path_buf()]);

    let result = file_collector
      .collect_files(Some(&[root_dir_path.to_path_buf()]))
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "git.js",
      "node_modules.js",
      "vendor.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test ignoring the .git and node_modules folder
    let file_collector = file_collector
      .ignore_git_folder()
      .ignore_node_modules()
      .ignore_vendor_folder();
    let result = file_collector
      .collect_files(Some(&[root_dir_path.to_path_buf()]))
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test opting out of ignoring by specifying the dir
    let result = file_collector
      .collect_files(Some(&[
        root_dir_path.to_path_buf(),
        root_dir_path.to_path_buf().join("child/node_modules/"),
      ]))
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "node_modules.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);
  }

  #[test]
  fn test_collect_specifiers() {
    fn create_files(dir_path: &PathRef, files: &[&str]) {
      dir_path.create_dir_all();
      for f in files {
        dir_path.join(f).write("");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let predicate = |path: &Path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|f| !f.starts_with('.'))
        .unwrap_or(false)
    };

    let result = collect_specifiers(
      &FilesConfig {
        include: Some(vec![
          PathBuf::from("http://localhost:8080"),
          root_dir_path.to_path_buf(),
          PathBuf::from("https://localhost:8080".to_string()),
        ]),
        exclude: vec![ignore_dir_path.to_path_buf()],
      },
      predicate,
    )
    .unwrap();

    let root_dir_url =
      ModuleSpecifier::from_file_path(root_dir_path.canonicalize())
        .unwrap()
        .to_string();
    let expected: Vec<ModuleSpecifier> = [
      "http://localhost:8080",
      &format!("{root_dir_url}/a.ts"),
      &format!("{root_dir_url}/b.js"),
      &format!("{root_dir_url}/c.tsx"),
      &format!("{root_dir_url}/child/README.md"),
      &format!("{root_dir_url}/child/e.mjs"),
      &format!("{root_dir_url}/child/f.mjsx"),
      &format!("{root_dir_url}/d.jsx"),
      "https://localhost:8080",
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);

    let scheme = if cfg!(target_os = "windows") {
      "file:///"
    } else {
      "file://"
    };
    let result = collect_specifiers(
      &FilesConfig {
        include: Some(vec![PathBuf::from(format!(
          "{}{}",
          scheme,
          root_dir_path.join("child").to_string().replace('\\', "/")
        ))]),
        exclude: vec![],
      },
      predicate,
    )
    .unwrap();

    let expected: Vec<ModuleSpecifier> = [
      &format!("{root_dir_url}/child/README.md"),
      &format!("{root_dir_url}/child/e.mjs"),
      &format!("{root_dir_url}/child/f.mjsx"),
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);
  }

  #[tokio::test]
  async fn lax_fs_lock() {
    let temp_dir = TempDir::new();
    let lock_path = temp_dir.path().join("file.lock");
    let signal1 = Arc::new(Notify::new());
    let signal2 = Arc::new(Notify::new());
    let signal3 = Arc::new(Notify::new());
    let signal4 = Arc::new(Notify::new());
    tokio::spawn({
      let lock_path = lock_path.clone();
      let signal1 = signal1.clone();
      let signal2 = signal2.clone();
      let signal3 = signal3.clone();
      let signal4 = signal4.clone();
      let temp_dir = temp_dir.clone();
      async move {
        let flag =
          LaxSingleProcessFsFlag::lock(lock_path.to_path_buf(), "waiting")
            .await;
        signal1.notify_one();
        signal2.notified().await;
        tokio::time::sleep(Duration::from_millis(10)).await; // give the other thread time to acquire the lock
        temp_dir.write("file.txt", "update1");
        signal3.notify_one();
        signal4.notified().await;
        drop(flag);
      }
    });
    let signal5 = Arc::new(Notify::new());
    tokio::spawn({
      let temp_dir = temp_dir.clone();
      let signal5 = signal5.clone();
      async move {
        signal1.notified().await;
        signal2.notify_one();
        let flag =
          LaxSingleProcessFsFlag::lock(lock_path.to_path_buf(), "waiting")
            .await;
        temp_dir.write("file.txt", "update2");
        signal5.notify_one();
        drop(flag);
      }
    });

    signal3.notified().await;
    assert_eq!(temp_dir.read_to_string("file.txt"), "update1");
    signal4.notify_one();
    signal5.notified().await;
    assert_eq!(temp_dir.read_to_string("file.txt"), "update2");
  }

  #[tokio::test]
  async fn lax_fs_lock_ordered() {
    let temp_dir = TempDir::new();
    let lock_path = temp_dir.path().join("file.lock");
    let output_path = temp_dir.path().join("output");
    let expected_order = Arc::new(Mutex::new(Vec::new()));
    let count = 10;
    let mut tasks = Vec::with_capacity(count);

    std::fs::write(&output_path, "").unwrap();

    for i in 0..count {
      let lock_path = lock_path.clone();
      let output_path = output_path.clone();
      let expected_order = expected_order.clone();
      tasks.push(tokio::spawn(async move {
        let flag =
          LaxSingleProcessFsFlag::lock(lock_path.to_path_buf(), "waiting")
            .await;
        expected_order.lock().push(i.to_string());
        // be extremely racy
        let mut output = std::fs::read_to_string(&output_path).unwrap();
        if !output.is_empty() {
          output.push('\n');
        }
        output.push_str(&i.to_string());
        std::fs::write(&output_path, output).unwrap();
        drop(flag);
      }));
    }

    futures::future::join_all(tasks).await;
    let expected_output = expected_order.lock().join("\n");
    assert_eq!(
      std::fs::read_to_string(output_path).unwrap(),
      expected_output
    );
  }
}
