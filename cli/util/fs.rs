// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;
use deno_config::glob::WalkEntry;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::unsync::spawn_blocking;
use deno_core::ModuleSpecifier;
use sys_traits::FsCreateDirAll;
use sys_traits::FsDirEntry;
use sys_traits::FsSymlinkDir;

use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;

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

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  Ok(deno_path_util::strip_unc_prefix(path.canonicalize()?))
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
  deno_path_util::fs::canonicalize_path_maybe_not_exists(
    &CliSys::default(),
    path,
  )
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
/// Note: This ignores all .git and node_modules folders.
pub fn collect_specifiers(
  mut files: FilePatterns,
  vendor_folder: Option<PathBuf>,
  predicate: impl Fn(WalkEntry) -> bool,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let mut prepared = vec![];

  // break out the remote specifiers
  if let Some(include_mut) = &mut files.include {
    let includes = std::mem::take(include_mut);
    let path_or_patterns = includes.into_path_or_patterns();
    let mut result = Vec::with_capacity(path_or_patterns.len());
    for path_or_pattern in path_or_patterns {
      match path_or_pattern {
        PathOrPattern::Path(path) => {
          if path.is_dir() {
            result.push(PathOrPattern::Path(path));
          } else if !files.exclude.matches_path(&path) {
            let url = specifier_from_file_path(&path)?;
            prepared.push(url);
          }
        }
        PathOrPattern::NegatedPath(path) => {
          // add it back
          result.push(PathOrPattern::NegatedPath(path));
        }
        PathOrPattern::RemoteUrl(remote_url) => {
          prepared.push(remote_url);
        }
        PathOrPattern::Pattern(pattern) => {
          // add it back
          result.push(PathOrPattern::Pattern(pattern));
        }
      }
    }
    *include_mut = PathOrPatternSet::new(result);
  }

  let collected_files = FileCollector::new(predicate)
    .ignore_git_folder()
    .ignore_node_modules()
    .set_vendor_folder(vendor_folder)
    .collect_file_patterns(&CliSys::default(), files);
  let mut collected_files_as_urls = collected_files
    .iter()
    .map(|f| specifier_from_file_path(f).unwrap())
    .collect::<Vec<ModuleSpecifier>>();

  collected_files_as_urls.sort();
  prepared.extend(collected_files_as_urls);

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

/// Clones a directory to another directory. The exact method
/// is not guaranteed - it may be a hardlink, copy, or other platform-specific
/// operation.
///
/// Note: Does not handle symlinks.
pub fn clone_dir_recursive<
  TSys: sys_traits::FsCopy
    + sys_traits::FsCloneFile
    + sys_traits::FsCloneFile
    + sys_traits::FsCreateDir
    + sys_traits::FsHardLink
    + sys_traits::FsReadDir
    + sys_traits::FsRemoveFile
    + sys_traits::ThreadSleep,
>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), CopyDirRecursiveError> {
  if cfg!(target_vendor = "apple") {
    if let Some(parent) = to.parent() {
      sys.fs_create_dir_all(parent)?;
    }
    // Try to clone the whole directory
    if let Err(err) = sys.fs_clone_file(from, to) {
      if !matches!(
        err.kind(),
        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::Unsupported
      ) {
        log::debug!(
          "Failed to clone dir {:?} to {:?} via clonefile: {}",
          from,
          to,
          err
        );
      }
      // clonefile won't overwrite existing files, so if the dir exists
      // we need to handle it recursively.
      copy_dir_recursive(sys, from, to)?;
    }
  } else if let Err(e) = deno_npm_cache::hard_link_dir_recursive(sys, from, to)
  {
    log::debug!("Failed to hard link dir {:?} to {:?}: {}", from, to, e);
    copy_dir_recursive(sys, from, to)?;
  }

  Ok(())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CopyDirRecursiveError {
  #[class(inherit)]
  #[error("Creating {path}")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: Error,
  },
  #[class(inherit)]
  #[error("Reading {path}")]
  Reading {
    path: PathBuf,
    #[source]
    #[inherit]
    source: Error,
  },
  #[class(inherit)]
  #[error("Dir {from} to {to}")]
  Dir {
    from: PathBuf,
    to: PathBuf,
    #[source]
    #[inherit]
    source: Box<Self>,
  },
  #[class(inherit)]
  #[error("Copying {from} to {to}")]
  Copying {
    from: PathBuf,
    to: PathBuf,
    #[source]
    #[inherit]
    source: Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] Error),
}

/// Copies a directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn copy_dir_recursive<
  TSys: sys_traits::FsCopy
    + sys_traits::FsCloneFile
    + sys_traits::FsCreateDir
    + sys_traits::FsHardLink
    + sys_traits::FsReadDir,
>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), CopyDirRecursiveError> {
  sys.fs_create_dir_all(to).map_err(|source| {
    CopyDirRecursiveError::Creating {
      path: to.to_path_buf(),
      source,
    }
  })?;
  let read_dir =
    sys
      .fs_read_dir(from)
      .map_err(|source| CopyDirRecursiveError::Reading {
        path: from.to_path_buf(),
        source,
      })?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      copy_dir_recursive(sys, &new_from, &new_to).map_err(|source| {
        CopyDirRecursiveError::Dir {
          from: new_from.to_path_buf(),
          to: new_to.to_path_buf(),
          source: Box::new(source),
        }
      })?;
    } else if file_type.is_file() {
      sys.fs_copy(&new_from, &new_to).map_err(|source| {
        CopyDirRecursiveError::Copying {
          from: new_from.to_path_buf(),
          to: new_to.to_path_buf(),
          source,
        }
      })?;
    }
  }

  Ok(())
}

pub fn symlink_dir<TSys: sys_traits::BaseFsSymlinkDir>(
  sys: &TSys,
  oldpath: &Path,
  newpath: &Path,
) -> Result<(), Error> {
  let err_mapper = |err: Error, kind: Option<ErrorKind>| {
    Error::new(
      kind.unwrap_or_else(|| err.kind()),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };

  sys.fs_symlink_dir(oldpath, newpath).map_err(|err| {
    #[cfg(windows)]
    if let Some(code) = err.raw_os_error() {
      if code as u32 == winapi::shared::winerror::ERROR_PRIVILEGE_NOT_HELD
        || code as u32 == winapi::shared::winerror::ERROR_INVALID_FUNCTION
      {
        return err_mapper(err, Some(ErrorKind::PermissionDenied));
      }
    }
    err_mapper(err, None)
  })
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
    // kill the poll thread
    self.finished_token.cancel();
    // release the file lock
    if let Err(err) = fs3::FileExt::unlock(&self.fs_file) {
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
pub struct LaxSingleProcessFsFlag(
  #[allow(dead_code)] Option<LaxSingleProcessFsFlagInner>,
);

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
      .truncate(false)
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
              // at the whims of whatever is occurring on the runtime thread.
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

pub fn specifier_from_file_path(
  path: &Path,
) -> Result<ModuleSpecifier, AnyError> {
  ModuleSpecifier::from_file_path(path)
    .map_err(|_| anyhow!("Invalid file path '{}'", path.display()))
}

#[cfg(test)]
mod tests {
  use deno_core::futures;
  use deno_core::parking_lot::Mutex;
  use deno_path_util::normalize_path;
  use pretty_assertions::assert_eq;
  use test_util::PathRef;
  use test_util::TempDir;
  use tokio::sync::Notify;

  use super::*;

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

    let predicate = |e: WalkEntry| {
      // exclude dotfiles
      e.path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|f| !f.starts_with('.'))
        .unwrap_or(false)
    };

    let result = collect_specifiers(
      FilePatterns {
        base: root_dir_path.to_path_buf(),
        include: Some(
          PathOrPatternSet::from_include_relative_path_or_patterns(
            root_dir_path.as_path(),
            &[
              "http://localhost:8080".to_string(),
              "./".to_string(),
              "https://localhost:8080".to_string(),
            ],
          )
          .unwrap(),
        ),
        exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
          ignore_dir_path.to_path_buf(),
        )]),
      },
      None,
      predicate,
    )
    .unwrap();

    let root_dir_url = ModuleSpecifier::from_file_path(&root_dir_path)
      .unwrap()
      .to_string();
    let expected = vec![
      "http://localhost:8080/".to_string(),
      "https://localhost:8080/".to_string(),
      format!("{root_dir_url}/a.ts"),
      format!("{root_dir_url}/b.js"),
      format!("{root_dir_url}/c.tsx"),
      format!("{root_dir_url}/child/README.md"),
      format!("{root_dir_url}/child/e.mjs"),
      format!("{root_dir_url}/child/f.mjsx"),
      format!("{root_dir_url}/d.jsx"),
    ];

    assert_eq!(
      result
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>(),
      expected
    );

    let scheme = if cfg!(target_os = "windows") {
      "file:///"
    } else {
      "file://"
    };
    let result = collect_specifiers(
      FilePatterns {
        base: root_dir_path.to_path_buf(),
        include: Some(PathOrPatternSet::new(vec![PathOrPattern::new(
          &format!(
            "{}{}",
            scheme,
            root_dir_path.join("child").to_string().replace('\\', "/")
          ),
        )
        .unwrap()])),
        exclude: Default::default(),
      },
      None,
      predicate,
    )
    .unwrap();

    let expected = vec![
      format!("{root_dir_url}/child/README.md"),
      format!("{root_dir_url}/child/e.mjs"),
      format!("{root_dir_url}/child/f.mjsx"),
    ];

    assert_eq!(
      result
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>(),
      expected
    );
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
