// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;

use sys_traits::FsDirEntry;
use sys_traits::FsSymlinkDir;
use sys_traits::PathsInErrorsExt;

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
/// Note: Does not copy source symlinks and rejects symlinked destinations.
pub fn clone_dir_recursive<TSys: CloneDirRecursiveSys>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  deno_npm_cache::ensure_not_symlink(sys.as_ref(), to)?;
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
      copy_dir_recursive(sys.as_ref(), from, to)?;
    }
  } else if let Err(e) =
    deno_npm_cache::hard_link_dir_recursive(sys.as_ref(), from, to)
  {
    log::debug!("Failed to hard link dir {:?} to {:?}: {}", from, to, e);
    copy_dir_recursive(sys.as_ref(), from, to)?;
  }

  Ok(())
}

#[sys_traits::auto_impl]
pub trait CopyDirRecursiveSys:
  sys_traits::FsCopy
  + sys_traits::FsCloneFile
  + sys_traits::FsCreateDir
  + sys_traits::FsHardLink
  + sys_traits::FsMetadata
  + sys_traits::FsReadDir
  + sys_traits::FsRemoveFile
{
}

/// Copies a directory to another directory.
///
/// Note: Does not copy source symlinks and rejects symlinked destinations.
pub fn copy_dir_recursive<TSys: CopyDirRecursiveSys>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  deno_npm_cache::create_dir_all_no_symlink(sys.as_ref(), to)?;
  let read_dir = sys.fs_read_dir(from)?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      copy_dir_recursive(sys.as_ref(), &new_from, &new_to)?;
    } else if file_type.is_file() {
      // Remove any existing file first so the copy writes a new inode.
      // The destination may be a hardlink to another path (for example,
      // esbuild's install script hardlinks its platform package's binary
      // over its own JS shim) and copying in place would write through
      // the link, corrupting the file at its other paths. Removing first
      // also breaks hardlinks to currently-executing binaries (ETXTBSY).
      let _ = sys.fs_remove_file(&new_to);
      sys.fs_copy(&new_from, &new_to)?;
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
      && (code as u32
        == windows_sys::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD
        || code as u32
          == windows_sys::Win32::Foundation::ERROR_INVALID_FUNCTION)
    {
      return err_mapper(err, Some(ErrorKind::PermissionDenied));
    }
    err_mapper(err, None)
  })
}

#[cfg(test)]
mod test {
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsHardLink;
  use sys_traits::FsRead;
  use sys_traits::FsSymlinkDir;
  use sys_traits::FsWrite;
  use test_util::TempDir;

  use super::*;

  #[test]
  fn copy_dir_recursive_replaces_hardlinked_files() {
    let temp_dir = TempDir::new();
    let sys = sys_traits::impls::RealSys;
    let root = temp_dir.path().to_path_buf();
    let from = root.join("from");
    let to = root.join("to");
    sys.fs_create_dir_all(&from).unwrap();
    sys.fs_create_dir_all(&to).unwrap();
    sys.fs_write(from.join("file"), "from contents").unwrap();
    // simulate what a lifecycle script like esbuild's install script does:
    // hardlink another file (e.g. a platform package's binary) over a file
    // that a future re-install will copy over again
    let binary = root.join("binary");
    sys.fs_write(&binary, "binary contents").unwrap();
    sys.fs_hard_link(&binary, to.join("file")).unwrap();

    copy_dir_recursive(&sys, &from, &to).unwrap();

    // the destination now has the copied contents on a new inode
    assert_eq!(
      sys.fs_read_to_string(to.join("file")).unwrap(),
      "from contents"
    );
    // and the other end of the hardlink was not written through
    assert_eq!(sys.fs_read_to_string(&binary).unwrap(), "binary contents");
  }

  #[test]
  fn copy_dir_recursive_rejects_symlink_destination_directory() {
    let sys = sys_traits::impls::InMemorySys::default();
    let from = Path::new("/from");
    let to = Path::new("/to");
    let target = Path::new("/target");
    sys.fs_create_dir_all(from).unwrap();
    sys.fs_create_dir_all(target).unwrap();
    sys.fs_write(from.join("file"), "package contents").unwrap();
    sys.fs_symlink_dir(target, to).unwrap();

    let err = copy_dir_recursive(&sys, from, to).unwrap_err();

    assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    assert!(sys.fs_read_to_string(target.join("file")).is_err());
  }
}
