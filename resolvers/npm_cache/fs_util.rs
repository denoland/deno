// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use sys_traits::FsCreateDirAll;
use sys_traits::FsDirEntry;
use sys_traits::FsHardLink;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::ThreadSleep;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HardLinkDirRecursiveError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error("Creating {path}")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Creating {path}")]
  Reading {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
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
  #[error(transparent)]
  HardLinkFile(#[from] HardLinkFileError),
}

/// Hardlinks the files in one directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn hard_link_dir_recursive<
  TSys: FsCreateDirAll + FsHardLink + FsReadDir + FsRemoveFile + ThreadSleep,
>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), HardLinkDirRecursiveError> {
  sys.fs_create_dir_all(to).map_err(|source| {
    HardLinkDirRecursiveError::Creating {
      path: to.to_path_buf(),
      source,
    }
  })?;
  let read_dir = sys.fs_read_dir(from).map_err(|source| {
    HardLinkDirRecursiveError::Reading {
      path: from.to_path_buf(),
      source,
    }
  })?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      hard_link_dir_recursive(sys, &new_from, &new_to).map_err(|source| {
        HardLinkDirRecursiveError::Dir {
          from: new_from.to_path_buf(),
          to: new_to.to_path_buf(),
          source: Box::new(source),
        }
      })?;
    } else if file_type.is_file() {
      hard_link_file(sys, &new_from, &new_to)?;
    }
  }

  Ok(())
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HardLinkFileError {
  #[class(inherit)]
  #[error("Removing file to hard link {from} to {to}")]
  RemoveFileToHardLink {
    from: PathBuf,
    to: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Hard linking {from} to {to}")]
  HardLinking {
    from: PathBuf,
    to: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

/// Hardlinks a file from one location to another.
pub fn hard_link_file<TSys: FsHardLink + FsRemoveFile + ThreadSleep>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), HardLinkFileError> {
  // note: chance for race conditions here between attempting to create,
  // then removing, then attempting to create. There doesn't seem to be
  // a way to hard link with overwriting in Rust, but maybe there is some
  // way with platform specific code. The workaround here is to handle
  // scenarios where something else might create or remove files.
  if let Err(err) = sys.fs_hard_link(from, to) {
    if err.kind() == ErrorKind::AlreadyExists {
      if let Err(err) = sys.fs_remove_file(to) {
        if err.kind() == ErrorKind::NotFound {
          // Assume another process/thread created this hard link to the file we are wanting
          // to remove then sleep a little bit to let the other process/thread move ahead
          // faster to reduce contention.
          sys.thread_sleep(Duration::from_millis(10));
        } else {
          return Err(HardLinkFileError::RemoveFileToHardLink {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source: err,
          });
        }
      }

      // Always attempt to recreate the hardlink. In contention scenarios, the other process
      // might have been killed or exited after removing the file, but before creating the hardlink
      if let Err(err) = sys.fs_hard_link(from, to) {
        // Assume another process/thread created this hard link to the file we are wanting
        // to now create then sleep a little bit to let the other process/thread move ahead
        // faster to reduce contention.
        if err.kind() == ErrorKind::AlreadyExists {
          sys.thread_sleep(Duration::from_millis(10));
        } else {
          return Err(HardLinkFileError::HardLinking {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source: err,
          });
        }
      }
    } else {
      return Err(HardLinkFileError::HardLinking {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source: err,
      });
    }
  }
  Ok(())
}
