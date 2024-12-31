// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Error as AnyError;
use sys_traits::FsCreateDirAll;
use sys_traits::FsDirEntry;
use sys_traits::FsHardLink;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::ThreadSleep;

/// Hardlinks the files in one directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn hard_link_dir_recursive<
  TSys: FsCreateDirAll + FsHardLink + FsReadDir + FsRemoveFile + ThreadSleep,
>(
  sys: &TSys,
  from: &Path,
  to: &Path,
) -> Result<(), AnyError> {
  sys
    .fs_create_dir_all(to)
    .with_context(|| format!("Creating {}", to.display()))?;
  let read_dir = sys
    .fs_read_dir(from)
    .with_context(|| format!("Reading {}", from.display()))?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      hard_link_dir_recursive(sys, &new_from, &new_to).with_context(|| {
        format!("Dir {} to {}", new_from.display(), new_to.display())
      })?;
    } else if file_type.is_file() {
      // note: chance for race conditions here between attempting to create,
      // then removing, then attempting to create. There doesn't seem to be
      // a way to hard link with overwriting in Rust, but maybe there is some
      // way with platform specific code. The workaround here is to handle
      // scenarios where something else might create or remove files.
      if let Err(err) = sys.fs_hard_link(&new_from, &new_to) {
        if err.kind() == ErrorKind::AlreadyExists {
          if let Err(err) = sys.fs_remove_file(&new_to) {
            if err.kind() == ErrorKind::NotFound {
              // Assume another process/thread created this hard link to the file we are wanting
              // to remove then sleep a little bit to let the other process/thread move ahead
              // faster to reduce contention.
              sys.thread_sleep(Duration::from_millis(10));
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
          if let Err(err) = sys.fs_hard_link(&new_from, &new_to) {
            // Assume another process/thread created this hard link to the file we are wanting
            // to now create then sleep a little bit to let the other process/thread move ahead
            // faster to reduce contention.
            if err.kind() == ErrorKind::AlreadyExists {
              sys.thread_sleep(Duration::from_millis(10));
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
