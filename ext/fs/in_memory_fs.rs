// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Allow using Arc for this module.
#![allow(clippy::disallowed_types)]

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::parking_lot::Mutex;
use deno_io::fs::File;
use deno_io::fs::FsError;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;
use deno_path_util::normalize_path;

use crate::interface::AccessCheckCb;
use crate::interface::FsDirEntry;
use crate::interface::FsFileType;
use crate::FileSystem;
use crate::OpenOptions;

#[derive(Debug)]
enum PathEntry {
  Dir,
  File(Vec<u8>),
}

/// A very basic in-memory file system useful for swapping out in
/// the place of a RealFs for testing purposes.
///
/// Please develop this out as you need functionality.
#[derive(Debug, Default)]
pub struct InMemoryFs {
  entries: Mutex<HashMap<PathBuf, Arc<PathEntry>>>,
}

impl InMemoryFs {
  pub fn setup_text_files(&self, files: Vec<(String, String)>) {
    for (path, text) in files {
      let path = PathBuf::from(path);
      self.mkdir_sync(path.parent().unwrap(), true, None).unwrap();
      self
        .write_file_sync(
          &path,
          OpenOptions::write(true, false, false, None),
          None,
          &text.into_bytes(),
        )
        .unwrap();
    }
  }

  fn get_entry(&self, path: &Path) -> Option<Arc<PathEntry>> {
    let path = normalize_path(path);
    self.entries.lock().get(&path).cloned()
  }
}

#[async_trait::async_trait(?Send)]
impl FileSystem for InMemoryFs {
  fn cwd(&self) -> FsResult<PathBuf> {
    Err(FsError::NotSupported)
  }

  fn tmp_dir(&self) -> FsResult<PathBuf> {
    Err(FsError::NotSupported)
  }

  fn chdir(&self, _path: &Path) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn umask(&self, _mask: Option<u32>) -> FsResult<u32> {
    Err(FsError::NotSupported)
  }

  fn open_sync(
    &self,
    _path: &Path,
    _options: OpenOptions,
    _access_check: Option<AccessCheckCb>,
  ) -> FsResult<Rc<dyn File>> {
    Err(FsError::NotSupported)
  }
  async fn open_async<'a>(
    &'a self,
    path: PathBuf,
    options: OpenOptions,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Rc<dyn File>> {
    self.open_sync(&path, options, access_check)
  }

  fn mkdir_sync(
    &self,
    path: &Path,
    recursive: bool,
    _mode: Option<u32>,
  ) -> FsResult<()> {
    let path = normalize_path(path);

    if let Some(parent) = path.parent() {
      let entry = self.entries.lock().get(parent).cloned();
      match entry {
        Some(entry) => match &*entry {
          PathEntry::File(_) => {
            return Err(FsError::Io(Error::new(
              ErrorKind::InvalidInput,
              "Parent is a file",
            )))
          }
          PathEntry::Dir => {}
        },
        None => {
          if recursive {
            self.mkdir_sync(parent, true, None)?;
          } else {
            return Err(FsError::Io(Error::new(
              ErrorKind::NotFound,
              "Not found",
            )));
          }
        }
      }
    }

    let entry = self.entries.lock().get(&path).cloned();
    match entry {
      Some(entry) => match &*entry {
        PathEntry::File(_) => Err(FsError::Io(Error::new(
          ErrorKind::InvalidInput,
          "Is a file",
        ))),
        PathEntry::Dir => Ok(()),
      },
      None => {
        self.entries.lock().insert(path, Arc::new(PathEntry::Dir));
        Ok(())
      }
    }
  }
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()> {
    self.mkdir_sync(&path, recursive, mode)
  }

  fn chmod_sync(&self, _path: &Path, _mode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chmod_async(&self, path: PathBuf, mode: u32) -> FsResult<()> {
    self.chmod_sync(&path, mode)
  }

  fn chown_sync(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.chown_sync(&path, uid, gid)
  }

  fn lchown_sync(
    &self,
    _path: &Path,
    _uid: Option<u32>,
    _gid: Option<u32>,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  async fn lchown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.lchown_sync(&path, uid, gid)
  }

  fn remove_sync(&self, _path: &Path, _recursive: bool) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn remove_async(&self, path: PathBuf, recursive: bool) -> FsResult<()> {
    self.remove_sync(&path, recursive)
  }

  fn copy_file_sync(&self, _from: &Path, _to: &Path) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn copy_file_async(&self, from: PathBuf, to: PathBuf) -> FsResult<()> {
    self.copy_file_sync(&from, &to)
  }

  fn cp_sync(&self, _from: &Path, _to: &Path) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn cp_async(&self, from: PathBuf, to: PathBuf) -> FsResult<()> {
    self.cp_sync(&from, &to)
  }

  fn stat_sync(&self, path: &Path) -> FsResult<FsStat> {
    let entry = self.get_entry(path);
    match entry {
      Some(entry) => match &*entry {
        PathEntry::Dir => Ok(FsStat {
          is_file: false,
          is_directory: true,
          is_symlink: false,
          size: 0,
          mtime: None,
          atime: None,
          birthtime: None,
          dev: 0,
          ino: 0,
          mode: 0,
          nlink: 0,
          uid: 0,
          gid: 0,
          rdev: 0,
          blksize: 0,
          blocks: 0,
          is_block_device: false,
          is_char_device: false,
          is_fifo: false,
          is_socket: false,
        }),
        PathEntry::File(data) => Ok(FsStat {
          is_file: true,
          is_directory: false,
          is_symlink: false,
          size: data.len() as u64,
          mtime: None,
          atime: None,
          birthtime: None,
          dev: 0,
          ino: 0,
          mode: 0,
          nlink: 0,
          uid: 0,
          gid: 0,
          rdev: 0,
          blksize: 0,
          blocks: 0,
          is_block_device: false,
          is_char_device: false,
          is_fifo: false,
          is_socket: false,
        }),
      },
      None => Err(FsError::Io(Error::new(ErrorKind::NotFound, "Not found"))),
    }
  }
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    self.stat_sync(&path)
  }

  fn lstat_sync(&self, _path: &Path) -> FsResult<FsStat> {
    Err(FsError::NotSupported)
  }
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    self.lstat_sync(&path)
  }

  fn realpath_sync(&self, _path: &Path) -> FsResult<PathBuf> {
    Err(FsError::NotSupported)
  }
  async fn realpath_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    self.realpath_sync(&path)
  }

  fn read_dir_sync(&self, _path: &Path) -> FsResult<Vec<FsDirEntry>> {
    Err(FsError::NotSupported)
  }
  async fn read_dir_async(&self, path: PathBuf) -> FsResult<Vec<FsDirEntry>> {
    self.read_dir_sync(&path)
  }

  fn rename_sync(&self, _oldpath: &Path, _newpath: &Path) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn rename_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    self.rename_sync(&oldpath, &newpath)
  }

  fn link_sync(&self, _oldpath: &Path, _newpath: &Path) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn link_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    self.link_sync(&oldpath, &newpath)
  }

  fn symlink_sync(
    &self,
    _oldpath: &Path,
    _newpath: &Path,
    _file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn symlink_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    self.symlink_sync(&oldpath, &newpath, file_type)
  }

  fn read_link_sync(&self, _path: &Path) -> FsResult<PathBuf> {
    Err(FsError::NotSupported)
  }
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    self.read_link_sync(&path)
  }

  fn truncate_sync(&self, _path: &Path, _len: u64) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn truncate_async(&self, path: PathBuf, len: u64) -> FsResult<()> {
    self.truncate_sync(&path, len)
  }

  fn utime_sync(
    &self,
    _path: &Path,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn utime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.utime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
  }

  fn lutime_sync(
    &self,
    _path: &Path,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn lutime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.lutime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
  }

  fn write_file_sync(
    &self,
    path: &Path,
    options: OpenOptions,
    _access_check: Option<AccessCheckCb>,
    data: &[u8],
  ) -> FsResult<()> {
    let path = normalize_path(path);
    let has_parent_dir = path
      .parent()
      .and_then(|parent| self.get_entry(parent))
      .map(|e| matches!(*e, PathEntry::Dir))
      .unwrap_or(false);
    if !has_parent_dir {
      return Err(FsError::Io(Error::new(
        ErrorKind::NotFound,
        "Parent directory does not exist",
      )));
    }
    let mut entries = self.entries.lock();
    let entry = entries.entry(path.clone());
    match entry {
      Entry::Occupied(mut entry) => {
        if let PathEntry::File(existing_data) = &**entry.get() {
          if options.create_new {
            return Err(FsError::Io(Error::new(
              ErrorKind::AlreadyExists,
              "File already exists",
            )));
          }
          if options.append {
            let mut new_data = existing_data.clone();
            new_data.extend_from_slice(data);
            entry.insert(Arc::new(PathEntry::File(new_data)));
          } else {
            entry.insert(Arc::new(PathEntry::File(data.to_vec())));
          }
          Ok(())
        } else {
          Err(FsError::Io(Error::new(
            ErrorKind::InvalidInput,
            "Not a file",
          )))
        }
      }
      Entry::Vacant(entry) => {
        entry.insert(Arc::new(PathEntry::File(data.to_vec())));
        Ok(())
      }
    }
  }

  async fn write_file_async<'a>(
    &'a self,
    path: PathBuf,
    options: OpenOptions,
    access_check: Option<AccessCheckCb<'a>>,
    data: Vec<u8>,
  ) -> FsResult<()> {
    self.write_file_sync(&path, options, access_check, &data)
  }

  fn read_file_sync(
    &self,
    path: &Path,
    _access_check: Option<AccessCheckCb>,
  ) -> FsResult<Vec<u8>> {
    let entry = self.get_entry(path);
    match entry {
      Some(entry) => match &*entry {
        PathEntry::File(data) => Ok(data.clone()),
        PathEntry::Dir => Err(FsError::Io(Error::new(
          ErrorKind::InvalidInput,
          "Is a directory",
        ))),
      },
      None => Err(FsError::Io(Error::new(ErrorKind::NotFound, "Not found"))),
    }
  }
  async fn read_file_async<'a>(
    &'a self,
    path: PathBuf,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Vec<u8>> {
    self.read_file_sync(&path, access_check)
  }
}
