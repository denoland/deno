// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use sys_traits::FsCreateDirAll;
use sys_traits::FsDirEntry;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::PathsInErrorsExt;
use sys_traits::ThreadSleep;

#[sys_traits::auto_impl]
pub trait HardLinkDirRecursiveSys:
  HardLinkFileSys + FsCreateDirAll + FsMetadata + FsReadDir
{
}

/// Hardlinks the files in one directory to another directory.
///
/// Note: Does not hardlink source symlinks and rejects symlinked destinations.
pub fn hard_link_dir_recursive<TSys: HardLinkDirRecursiveSys>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
  create_dir_all_no_symlink(sys.as_ref(), to)?;
  let read_dir = sys.fs_read_dir(from)?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      hard_link_dir_recursive(sys.as_ref(), &new_from, &new_to)?;
    } else if file_type.is_file() {
      hard_link_file(sys.as_ref(), &new_from, &new_to)?;
    }
  }

  Ok(())
}

fn create_dir_all_no_symlink<TSys>(
  sys: &TSys,
  path: &Path,
) -> Result<(), std::io::Error>
where
  TSys: FsCreateDirAll + FsMetadata,
{
  ensure_not_symlink(sys, path)?;
  sys.fs_create_dir_all(path)?;
  ensure_not_symlink(sys, path)
}

fn ensure_not_symlink<TSys>(
  sys: &TSys,
  path: &Path,
) -> Result<(), std::io::Error>
where
  TSys: FsMetadata,
{
  match sys.fs_symlink_metadata(path) {
    Ok(metadata) if metadata.file_type().is_symlink() => {
      Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "refusing to materialize package into symlinked directory",
      ))
    }
    Ok(_) => Ok(()),
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
    Err(err) => Err(err),
  }
}

#[sys_traits::auto_impl]
pub trait HardLinkFileSys: FsHardLink + FsRemoveFile + ThreadSleep {}

/// Hardlinks a file from one location to another.
pub fn hard_link_file<TSys: HardLinkFileSys>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), std::io::Error> {
  let sys = sys.with_paths_in_errors();
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
          sys.as_ref().thread_sleep(Duration::from_millis(10));
        } else {
          // Note: on overlay filesystems (e.g. Podman's fuse-overlayfs),
          // this can fail with ETXTBSY if the file is being executed.
          // Callers should handle this by falling back to copy.
          return Err(err);
        }
      }

      // Always attempt to recreate the hardlink. In contention scenarios, the other process
      // might have been killed or exited after removing the file, but before creating the hardlink
      if let Err(err) = sys.fs_hard_link(from, to) {
        // Assume another process/thread created this hard link to the file we are wanting
        // to now create then sleep a little bit to let the other process/thread move ahead
        // faster to reduce contention.
        if err.kind() == ErrorKind::AlreadyExists {
          sys.as_ref().thread_sleep(Duration::from_millis(10));
        } else {
          return Err(err);
        }
      }
    } else {
      return Err(err);
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use sys_traits::FsCreateDirAll;
  use sys_traits::FsRead;
  use sys_traits::FsSymlinkDir;
  use sys_traits::FsWrite;

  use super::*;

  #[test]
  fn hard_link_dir_recursive_rejects_symlink_destination_directory() {
    let sys = sys_traits::impls::InMemorySys::default();
    let from = Path::new("/from");
    let to = Path::new("/to");
    let target = Path::new("/target");
    sys.fs_create_dir_all(from).unwrap();
    sys.fs_create_dir_all(target).unwrap();
    sys.fs_write(from.join("file"), "package contents").unwrap();
    sys.fs_symlink_dir(target, to).unwrap();

    let err = hard_link_dir_recursive(&sys, from, to).unwrap_err();

    assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    assert!(sys.fs_read_to_string(target.join("file")).is_err());
  }
}
