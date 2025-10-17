// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::FsCreateDirAll;
use sys_traits::FsDirEntry;
use sys_traits::FsSymlinkDir;

#[sys_traits::auto_impl]
pub trait CloneDirRecursiveSys:
  CopyDirRecursiveSys
  + sys_traits::FsCreateDirAll
  + sys_traits::FsRemoveFile
  + sys_traits::FsRemoveDirAll
  + sys_traits::ThreadSleep
{
}

/// Clones a directory to another directory. The exact method
/// is not guaranteed - it may be a hardlink, copy, or other platform-specific
/// operation.
///
/// Note: Does not handle symlinks.
pub fn clone_dir_recursive<TSys: CloneDirRecursiveSys>(
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

#[sys_traits::auto_impl]
pub trait CopyDirRecursiveSys:
  sys_traits::FsCopy
  + sys_traits::FsCloneFile
  + sys_traits::FsCreateDir
  + sys_traits::FsHardLink
  + sys_traits::FsReadDir
{
}

/// Copies a directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn copy_dir_recursive<TSys: CopyDirRecursiveSys>(
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
    if let Some(code) = err.raw_os_error()
      && (code as u32 == winapi::shared::winerror::ERROR_PRIVILEGE_NOT_HELD
        || code as u32 == winapi::shared::winerror::ERROR_INVALID_FUNCTION)
    {
      return err_mapper(err, Some(ErrorKind::PermissionDenied));
    }
    err_mapper(err, None)
  })
}
