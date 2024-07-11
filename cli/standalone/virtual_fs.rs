// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::ResourceHandleFd;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_io;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::util;
use crate::util::fs::canonicalize_path;

#[derive(Error, Debug)]
#[error(
  "Failed to strip prefix '{}' from '{}'", root_path.display(), target.display()
)]
pub struct StripRootError {
  root_path: PathBuf,
  target: PathBuf,
}

pub struct VfsBuilder {
  root_path: PathBuf,
  root_dir: VirtualDirectory,
  files: Vec<Vec<u8>>,
  current_offset: u64,
  file_offsets: HashMap<String, u64>,
}

impl VfsBuilder {
  pub fn new(root_path: PathBuf) -> Result<Self, AnyError> {
    let root_path = canonicalize_path(&root_path)?;
    log::debug!("Building vfs with root '{}'", root_path.display());
    Ok(Self {
      root_dir: VirtualDirectory {
        name: root_path
          .file_stem()
          .map(|s| s.to_string_lossy().into_owned())
          .unwrap_or("root".to_string()),
        entries: Vec::new(),
      },
      root_path,
      files: Vec::new(),
      current_offset: 0,
      file_offsets: Default::default(),
    })
  }

  pub fn with_root_dir<R>(
    &mut self,
    with_root: impl FnOnce(&mut VirtualDirectory) -> R,
  ) -> R {
    with_root(&mut self.root_dir)
  }

  pub fn add_dir_recursive(&mut self, path: &Path) -> Result<(), AnyError> {
    let target_path = canonicalize_path(path)?;
    if path != target_path {
      self.add_symlink(path, &target_path)?;
    }
    self.add_dir_recursive_internal(&target_path)
  }

  fn add_dir_recursive_internal(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    self.add_dir(path)?;
    let read_dir = std::fs::read_dir(path)
      .with_context(|| format!("Reading {}", path.display()))?;

    for entry in read_dir {
      let entry = entry?;
      let file_type = entry.file_type()?;
      let path = entry.path();

      if file_type.is_dir() {
        self.add_dir_recursive_internal(&path)?;
      } else if file_type.is_file() {
        self.add_file_at_path_not_symlink(&path)?;
      } else if file_type.is_symlink() {
        match util::fs::canonicalize_path(&path) {
          Ok(target) => {
            if let Err(StripRootError { .. }) = self.add_symlink(&path, &target)
            {
              if target.is_file() {
                // this may change behavior, so warn the user about it
                log::warn!(
                  "{} Symlink target is outside '{}'. Inlining symlink at '{}' to '{}' as file.",
                  crate::colors::yellow("Warning"),
                  self.root_path.display(),
                  path.display(),
                  target.display(),
                );
                // inline the symlink and make the target file
                let file_bytes = std::fs::read(&target)
                  .with_context(|| format!("Reading {}", path.display()))?;
                self.add_file(&path, file_bytes)?;
              } else {
                log::warn!(
                  "{} Symlink target is outside '{}'. Excluding symlink at '{}' with target '{}'.",
                  crate::colors::yellow("Warning"),
                  self.root_path.display(),
                  path.display(),
                  target.display(),
                );
              }
            }
          }
          Err(err) => {
            log::warn!(
              "{} Failed resolving symlink. Ignoring.\n    Path: {}\n    Message: {:#}",
              crate::colors::yellow("Warning"),
              path.display(),
              err
            );
          }
        }
      }
    }

    Ok(())
  }

  fn add_dir(
    &mut self,
    path: &Path,
  ) -> Result<&mut VirtualDirectory, StripRootError> {
    log::debug!("Ensuring directory '{}'", path.display());
    let path = self.path_relative_root(path)?;
    let mut current_dir = &mut self.root_dir;

    for component in path.components() {
      let name = component.as_os_str().to_string_lossy();
      let index = match current_dir
        .entries
        .binary_search_by(|e| e.name().cmp(&name))
      {
        Ok(index) => index,
        Err(insert_index) => {
          current_dir.entries.insert(
            insert_index,
            VfsEntry::Dir(VirtualDirectory {
              name: name.to_string(),
              entries: Vec::new(),
            }),
          );
          insert_index
        }
      };
      match &mut current_dir.entries[index] {
        VfsEntry::Dir(dir) => {
          current_dir = dir;
        }
        _ => unreachable!(),
      };
    }

    Ok(current_dir)
  }

  pub fn add_file_at_path(&mut self, path: &Path) -> Result<(), AnyError> {
    let target_path = canonicalize_path(path)?;
    if target_path != path {
      self.add_symlink(path, &target_path)?;
    }
    self.add_file_at_path_not_symlink(&target_path)
  }

  pub fn add_file_at_path_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    let file_bytes = std::fs::read(path)
      .with_context(|| format!("Reading {}", path.display()))?;
    self.add_file(path, file_bytes)
  }

  fn add_file(&mut self, path: &Path, data: Vec<u8>) -> Result<(), AnyError> {
    log::debug!("Adding file '{}'", path.display());
    let checksum = util::checksum::gen(&[&data]);
    let offset = if let Some(offset) = self.file_offsets.get(&checksum) {
      // duplicate file, reuse an old offset
      *offset
    } else {
      self.file_offsets.insert(checksum, self.current_offset);
      self.current_offset
    };

    let dir = self.add_dir(path.parent().unwrap())?;
    let name = path.file_name().unwrap().to_string_lossy();
    let data_len = data.len();
    match dir.entries.binary_search_by(|e| e.name().cmp(&name)) {
      Ok(_) => {
        // already added, just ignore
      }
      Err(insert_index) => {
        dir.entries.insert(
          insert_index,
          VfsEntry::File(VirtualFile {
            name: name.to_string(),
            offset,
            len: data.len() as u64,
          }),
        );
      }
    }

    // new file, update the list of files
    if self.current_offset == offset {
      self.files.push(data);
      self.current_offset += data_len as u64;
    }

    Ok(())
  }

  fn add_symlink(
    &mut self,
    path: &Path,
    target: &Path,
  ) -> Result<(), StripRootError> {
    log::debug!(
      "Adding symlink '{}' to '{}'",
      path.display(),
      target.display()
    );
    let dest = self.path_relative_root(target)?;
    if dest == self.path_relative_root(path)? {
      // it's the same, ignore
      return Ok(());
    }
    let dir = self.add_dir(path.parent().unwrap())?;
    let name = path.file_name().unwrap().to_string_lossy();
    match dir.entries.binary_search_by(|e| e.name().cmp(&name)) {
      Ok(_) => unreachable!(),
      Err(insert_index) => {
        dir.entries.insert(
          insert_index,
          VfsEntry::Symlink(VirtualSymlink {
            name: name.to_string(),
            dest_parts: dest
              .components()
              .map(|c| c.as_os_str().to_string_lossy().to_string())
              .collect::<Vec<_>>(),
          }),
        );
      }
    }
    Ok(())
  }

  pub fn into_dir_and_files(self) -> (VirtualDirectory, Vec<Vec<u8>>) {
    (self.root_dir, self.files)
  }

  fn path_relative_root(&self, path: &Path) -> Result<PathBuf, StripRootError> {
    match path.strip_prefix(&self.root_path) {
      Ok(p) => Ok(p.to_path_buf()),
      Err(_) => Err(StripRootError {
        root_path: self.root_path.clone(),
        target: path.to_path_buf(),
      }),
    }
  }
}

#[derive(Debug)]
enum VfsEntryRef<'a> {
  Dir(&'a VirtualDirectory),
  File(&'a VirtualFile),
  Symlink(&'a VirtualSymlink),
}

impl<'a> VfsEntryRef<'a> {
  pub fn as_fs_stat(&self) -> FsStat {
    match self {
      VfsEntryRef::Dir(_) => FsStat {
        is_directory: true,
        is_file: false,
        is_symlink: false,
        atime: None,
        birthtime: None,
        mtime: None,
        blksize: 0,
        size: 0,
        dev: 0,
        ino: 0,
        mode: 0,
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blocks: 0,
        is_block_device: false,
        is_char_device: false,
        is_fifo: false,
        is_socket: false,
      },
      VfsEntryRef::File(file) => FsStat {
        is_directory: false,
        is_file: true,
        is_symlink: false,
        atime: None,
        birthtime: None,
        mtime: None,
        blksize: 0,
        size: file.len,
        dev: 0,
        ino: 0,
        mode: 0,
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blocks: 0,
        is_block_device: false,
        is_char_device: false,
        is_fifo: false,
        is_socket: false,
      },
      VfsEntryRef::Symlink(_) => FsStat {
        is_directory: false,
        is_file: false,
        is_symlink: true,
        atime: None,
        birthtime: None,
        mtime: None,
        blksize: 0,
        size: 0,
        dev: 0,
        ino: 0,
        mode: 0,
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blocks: 0,
        is_block_device: false,
        is_char_device: false,
        is_fifo: false,
        is_socket: false,
      },
    }
  }
}

// todo(dsherret): we should store this more efficiently in the binary
#[derive(Debug, Serialize, Deserialize)]
pub enum VfsEntry {
  Dir(VirtualDirectory),
  File(VirtualFile),
  Symlink(VirtualSymlink),
}

impl VfsEntry {
  pub fn name(&self) -> &str {
    match self {
      VfsEntry::Dir(dir) => &dir.name,
      VfsEntry::File(file) => &file.name,
      VfsEntry::Symlink(symlink) => &symlink.name,
    }
  }

  fn as_ref(&self) -> VfsEntryRef {
    match self {
      VfsEntry::Dir(dir) => VfsEntryRef::Dir(dir),
      VfsEntry::File(file) => VfsEntryRef::File(file),
      VfsEntry::Symlink(symlink) => VfsEntryRef::Symlink(symlink),
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualDirectory {
  pub name: String,
  // should be sorted by name
  pub entries: Vec<VfsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFile {
  pub name: String,
  pub offset: u64,
  pub len: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualSymlink {
  pub name: String,
  pub dest_parts: Vec<String>,
}

impl VirtualSymlink {
  pub fn resolve_dest_from_root(&self, root: &Path) -> PathBuf {
    let mut dest = root.to_path_buf();
    for part in &self.dest_parts {
      dest.push(part);
    }
    dest
  }
}

#[derive(Debug)]
pub struct VfsRoot {
  pub dir: VirtualDirectory,
  pub root_path: PathBuf,
  pub start_file_offset: u64,
}

impl VfsRoot {
  fn find_entry<'a>(
    &'a self,
    path: &Path,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    self.find_entry_inner(path, &mut HashSet::new())
  }

  fn find_entry_inner<'a>(
    &'a self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    let mut path = Cow::Borrowed(path);
    loop {
      let (resolved_path, entry) =
        self.find_entry_no_follow_inner(&path, seen)?;
      match entry {
        VfsEntryRef::Symlink(symlink) => {
          if !seen.insert(path.to_path_buf()) {
            return Err(std::io::Error::new(
              std::io::ErrorKind::Other,
              "circular symlinks",
            ));
          }
          path = Cow::Owned(symlink.resolve_dest_from_root(&self.root_path));
        }
        _ => {
          return Ok((resolved_path, entry));
        }
      }
    }
  }

  fn find_entry_no_follow(
    &self,
    path: &Path,
  ) -> std::io::Result<(PathBuf, VfsEntryRef)> {
    self.find_entry_no_follow_inner(path, &mut HashSet::new())
  }

  fn find_entry_no_follow_inner<'a>(
    &'a self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    let relative_path = match path.strip_prefix(&self.root_path) {
      Ok(p) => p,
      Err(_) => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "path not found",
        ));
      }
    };
    let mut final_path = self.root_path.clone();
    let mut current_entry = VfsEntryRef::Dir(&self.dir);
    for component in relative_path.components() {
      let component = component.as_os_str().to_string_lossy();
      let current_dir = match current_entry {
        VfsEntryRef::Dir(dir) => {
          final_path.push(component.as_ref());
          dir
        }
        VfsEntryRef::Symlink(symlink) => {
          let dest = symlink.resolve_dest_from_root(&self.root_path);
          let (resolved_path, entry) = self.find_entry_inner(&dest, seen)?;
          final_path = resolved_path; // overwrite with the new resolved path
          match entry {
            VfsEntryRef::Dir(dir) => {
              final_path.push(component.as_ref());
              dir
            }
            _ => {
              return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "path not found",
              ));
            }
          }
        }
        _ => {
          return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path not found",
          ));
        }
      };
      match current_dir
        .entries
        .binary_search_by(|e| e.name().cmp(&component))
      {
        Ok(index) => {
          current_entry = current_dir.entries[index].as_ref();
        }
        Err(_) => {
          return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path not found",
          ));
        }
      }
    }

    Ok((final_path, current_entry))
  }
}

#[derive(Clone)]
struct FileBackedVfsFile {
  file: VirtualFile,
  pos: Arc<Mutex<u64>>,
  vfs: Arc<FileBackedVfs>,
}

impl FileBackedVfsFile {
  fn seek(&self, pos: SeekFrom) -> FsResult<u64> {
    match pos {
      SeekFrom::Start(pos) => {
        *self.pos.lock() = pos;
        Ok(pos)
      }
      SeekFrom::End(offset) => {
        if offset < 0 && -offset as u64 > self.file.len {
          let msg = "An attempt was made to move the file pointer before the beginning of the file.";
          Err(
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, msg)
              .into(),
          )
        } else {
          let mut current_pos = self.pos.lock();
          *current_pos = if offset >= 0 {
            self.file.len - (offset as u64)
          } else {
            self.file.len + (-offset as u64)
          };
          Ok(*current_pos)
        }
      }
      SeekFrom::Current(offset) => {
        let mut current_pos = self.pos.lock();
        if offset >= 0 {
          *current_pos += offset as u64;
        } else if -offset as u64 > *current_pos {
          return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "An attempt was made to move the file pointer before the beginning of the file.").into());
        } else {
          *current_pos -= -offset as u64;
        }
        Ok(*current_pos)
      }
    }
  }

  fn read_to_buf(&self, buf: &mut [u8]) -> FsResult<usize> {
    let pos = {
      let mut pos = self.pos.lock();
      let read_pos = *pos;
      // advance the position due to the read
      *pos = std::cmp::min(self.file.len, *pos + buf.len() as u64);
      read_pos
    };
    self
      .vfs
      .read_file(&self.file, pos, buf)
      .map_err(|err| err.into())
  }

  fn read_to_end(&self) -> FsResult<Vec<u8>> {
    let pos = {
      let mut pos = self.pos.lock();
      let read_pos = *pos;
      // todo(dsherret): should this always set it to the end of the file?
      if *pos < self.file.len {
        // advance the position due to the read
        *pos = self.file.len;
      }
      read_pos
    };
    if pos > self.file.len {
      return Ok(Vec::new());
    }
    let size = (self.file.len - pos) as usize;
    let mut buf = vec![0; size];
    self.vfs.read_file(&self.file, pos, &mut buf)?;
    Ok(buf)
  }
}

#[async_trait::async_trait(?Send)]
impl deno_io::fs::File for FileBackedVfsFile {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize> {
    self.read_to_buf(buf)
  }
  async fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)> {
    let inner = (*self).clone();
    tokio::task::spawn(async move {
      let nread = inner.read_to_buf(&mut buf)?;
      Ok((nread, buf))
    })
    .await?
  }

  fn write_sync(self: Rc<Self>, _buf: &[u8]) -> FsResult<usize> {
    Err(FsError::NotSupported)
  }
  async fn write(
    self: Rc<Self>,
    _buf: BufView,
  ) -> FsResult<deno_core::WriteOutcome> {
    Err(FsError::NotSupported)
  }

  fn write_all_sync(self: Rc<Self>, _buf: &[u8]) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn write_all(self: Rc<Self>, _buf: BufView) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn read_all_sync(self: Rc<Self>) -> FsResult<Vec<u8>> {
    self.read_to_end()
  }
  async fn read_all_async(self: Rc<Self>) -> FsResult<Vec<u8>> {
    let inner = (*self).clone();
    tokio::task::spawn_blocking(move || inner.read_to_end()).await?
  }

  fn chmod_sync(self: Rc<Self>, _pathmode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chmod_async(self: Rc<Self>, _mode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn seek_sync(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos)
  }
  async fn seek_async(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos)
  }

  fn datasync_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn datasync_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn sync_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn sync_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn stat_sync(self: Rc<Self>) -> FsResult<FsStat> {
    Err(FsError::NotSupported)
  }
  async fn stat_async(self: Rc<Self>) -> FsResult<FsStat> {
    Err(FsError::NotSupported)
  }

  fn lock_sync(self: Rc<Self>, _exclusive: bool) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn lock_async(self: Rc<Self>, _exclusive: bool) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn unlock_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn unlock_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn truncate_sync(self: Rc<Self>, _len: u64) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn truncate_async(self: Rc<Self>, _len: u64) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn utime_sync(
    self: Rc<Self>,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn utime_async(
    self: Rc<Self>,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  // lower level functionality
  fn as_stdio(self: Rc<Self>) -> FsResult<std::process::Stdio> {
    Err(FsError::NotSupported)
  }
  fn backing_fd(self: Rc<Self>) -> Option<ResourceHandleFd> {
    None
  }
  fn try_clone_inner(self: Rc<Self>) -> FsResult<Rc<dyn deno_io::fs::File>> {
    Ok(self)
  }
}

#[derive(Debug)]
pub struct FileBackedVfs {
  file: Mutex<File>,
  fs_root: VfsRoot,
}

impl FileBackedVfs {
  pub fn new(file: File, fs_root: VfsRoot) -> Self {
    Self {
      file: Mutex::new(file),
      fs_root,
    }
  }

  pub fn root(&self) -> &Path {
    &self.fs_root.root_path
  }

  pub fn is_path_within(&self, path: &Path) -> bool {
    path.starts_with(&self.fs_root.root_path)
  }

  pub fn open_file(
    self: &Arc<Self>,
    path: &Path,
  ) -> std::io::Result<Rc<dyn deno_io::fs::File>> {
    let file = self.file_entry(path)?;
    Ok(Rc::new(FileBackedVfsFile {
      file: file.clone(),
      vfs: self.clone(),
      pos: Default::default(),
    }))
  }

  pub fn read_dir(&self, path: &Path) -> std::io::Result<Vec<FsDirEntry>> {
    let dir = self.dir_entry(path)?;
    Ok(
      dir
        .entries
        .iter()
        .map(|entry| FsDirEntry {
          name: entry.name().to_string(),
          is_file: matches!(entry, VfsEntry::File(_)),
          is_directory: matches!(entry, VfsEntry::Dir(_)),
          is_symlink: matches!(entry, VfsEntry::Symlink(_)),
        })
        .collect(),
    )
  }

  pub fn read_dir_names(&self, path: &Path) -> std::io::Result<Vec<String>> {
    let dir = self.dir_entry(path)?;
    Ok(
      dir
        .entries
        .iter()
        .map(|entry| entry.name().to_string())
        .collect(),
    )
  }

  pub fn read_link(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (_, entry) = self.fs_root.find_entry_no_follow(path)?;
    match entry {
      VfsEntryRef::Symlink(symlink) => {
        Ok(symlink.resolve_dest_from_root(&self.fs_root.root_path))
      }
      VfsEntryRef::Dir(_) | VfsEntryRef::File(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "not a symlink",
      )),
    }
  }

  pub fn lstat(&self, path: &Path) -> std::io::Result<FsStat> {
    let (_, entry) = self.fs_root.find_entry_no_follow(path)?;
    Ok(entry.as_fs_stat())
  }

  pub fn stat(&self, path: &Path) -> std::io::Result<FsStat> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    Ok(entry.as_fs_stat())
  }

  pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (path, _) = self.fs_root.find_entry(path)?;
    Ok(path)
  }

  pub fn read_file_all(&self, file: &VirtualFile) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0; file.len as usize];
    self.read_file(file, 0, &mut buf)?;
    Ok(buf)
  }

  pub fn read_file(
    &self,
    file: &VirtualFile,
    pos: u64,
    buf: &mut [u8],
  ) -> std::io::Result<usize> {
    let mut fs_file = self.file.lock();
    fs_file.seek(SeekFrom::Start(
      self.fs_root.start_file_offset + file.offset + pos,
    ))?;
    fs_file.read(buf)
  }

  pub fn dir_entry(&self, path: &Path) -> std::io::Result<&VirtualDirectory> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    match entry {
      VfsEntryRef::Dir(dir) => Ok(dir),
      VfsEntryRef::Symlink(_) => unreachable!(),
      VfsEntryRef::File(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "path is a file",
      )),
    }
  }

  pub fn file_entry(&self, path: &Path) -> std::io::Result<&VirtualFile> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    match entry {
      VfsEntryRef::Dir(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "path is a directory",
      )),
      VfsEntryRef::Symlink(_) => unreachable!(),
      VfsEntryRef::File(file) => Ok(file),
    }
  }
}

#[cfg(test)]
mod test {
  use std::io::Write;
  use test_util::TempDir;

  use super::*;

  #[track_caller]
  fn read_file(vfs: &FileBackedVfs, path: &Path) -> String {
    let file = vfs.file_entry(path).unwrap();
    String::from_utf8(vfs.read_file_all(file).unwrap()).unwrap()
  }

  #[test]
  fn builds_and_uses_virtual_fs() {
    let temp_dir = TempDir::new();
    // we canonicalize the temp directory because the vfs builder
    // will canonicalize the root path
    let src_path = temp_dir.path().canonicalize().join("src");
    src_path.create_dir_all();
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new(src_path.clone()).unwrap();
    builder
      .add_file(&src_path.join("a.txt"), "data".into())
      .unwrap();
    builder
      .add_file(&src_path.join("b.txt"), "data".into())
      .unwrap();
    assert_eq!(builder.files.len(), 1); // because duplicate data
    builder
      .add_file(&src_path.join("c.txt"), "c".into())
      .unwrap();
    builder
      .add_file(&src_path.join("sub_dir").join("d.txt"), "d".into())
      .unwrap();
    builder
      .add_file(&src_path.join("e.txt"), "e".into())
      .unwrap();
    builder
      .add_symlink(
        &src_path.join("sub_dir").join("e.txt"),
        &src_path.join("e.txt"),
      )
      .unwrap();

    // get the virtual fs
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(read_file(&virtual_fs, &dest_path.join("a.txt")), "data");
    assert_eq!(read_file(&virtual_fs, &dest_path.join("b.txt")), "data");

    // attempt reading a symlink
    assert_eq!(
      read_file(&virtual_fs, &dest_path.join("sub_dir").join("e.txt")),
      "e",
    );

    // canonicalize symlink
    assert_eq!(
      virtual_fs
        .canonicalize(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap(),
      dest_path.join("e.txt"),
    );

    // metadata
    assert!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .is_symlink
    );
    assert!(
      virtual_fs
        .stat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .is_file
    );
    assert!(
      virtual_fs
        .stat(&dest_path.join("sub_dir"))
        .unwrap()
        .is_directory,
    );
    assert!(virtual_fs.stat(&dest_path.join("e.txt")).unwrap().is_file,);
  }

  #[test]
  fn test_include_dir_recursive() {
    let temp_dir = TempDir::new();
    let temp_dir_path = temp_dir.path().canonicalize();
    temp_dir.create_dir_all("src/nested/sub_dir");
    temp_dir.write("src/a.txt", "data");
    temp_dir.write("src/b.txt", "data");
    util::fs::symlink_dir(
      temp_dir_path.join("src/nested/sub_dir").as_path(),
      temp_dir_path.join("src/sub_dir_link").as_path(),
    )
    .unwrap();
    temp_dir.write("src/nested/sub_dir/c.txt", "c");

    // build and create the virtual fs
    let src_path = temp_dir_path.join("src").to_path_buf();
    let mut builder = VfsBuilder::new(src_path.clone()).unwrap();
    builder.add_dir_recursive(&src_path).unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(read_file(&virtual_fs, &dest_path.join("a.txt")), "data",);
    assert_eq!(read_file(&virtual_fs, &dest_path.join("b.txt")), "data",);

    assert_eq!(
      read_file(
        &virtual_fs,
        &dest_path.join("nested").join("sub_dir").join("c.txt")
      ),
      "c",
    );
    assert_eq!(
      read_file(&virtual_fs, &dest_path.join("sub_dir_link").join("c.txt")),
      "c",
    );
    assert!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir_link"))
        .unwrap()
        .is_symlink
    );

    assert_eq!(
      virtual_fs
        .canonicalize(&dest_path.join("sub_dir_link").join("c.txt"))
        .unwrap(),
      dest_path.join("nested").join("sub_dir").join("c.txt"),
    );
  }

  fn into_virtual_fs(
    builder: VfsBuilder,
    temp_dir: &TempDir,
  ) -> (PathBuf, FileBackedVfs) {
    let virtual_fs_file = temp_dir.path().join("virtual_fs");
    let (root_dir, files) = builder.into_dir_and_files();
    {
      let mut file = std::fs::File::create(&virtual_fs_file).unwrap();
      for file_data in &files {
        file.write_all(file_data).unwrap();
      }
    }
    let file = std::fs::File::open(&virtual_fs_file).unwrap();
    let dest_path = temp_dir.path().join("dest");
    (
      dest_path.to_path_buf(),
      FileBackedVfs::new(
        file,
        VfsRoot {
          dir: root_dir,
          root_path: dest_path.to_path_buf(),
          start_file_offset: 0,
        },
      ),
    )
  }

  #[test]
  fn circular_symlink() {
    let temp_dir = TempDir::new();
    let src_path = temp_dir.path().canonicalize().join("src");
    src_path.create_dir_all();
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new(src_path.clone()).unwrap();
    builder
      .add_symlink(&src_path.join("a.txt"), &src_path.join("b.txt"))
      .unwrap();
    builder
      .add_symlink(&src_path.join("b.txt"), &src_path.join("c.txt"))
      .unwrap();
    builder
      .add_symlink(&src_path.join("c.txt"), &src_path.join("a.txt"))
      .unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);
    assert_eq!(
      virtual_fs
        .file_entry(&dest_path.join("a.txt"))
        .err()
        .unwrap()
        .to_string(),
      "circular symlinks",
    );
    assert_eq!(
      virtual_fs.read_link(&dest_path.join("a.txt")).unwrap(),
      dest_path.join("b.txt")
    );
    assert_eq!(
      virtual_fs.read_link(&dest_path.join("b.txt")).unwrap(),
      dest_path.join("c.txt")
    );
  }

  #[tokio::test]
  async fn test_open_file() {
    let temp_dir = TempDir::new();
    let temp_path = temp_dir.path().canonicalize();
    let mut builder = VfsBuilder::new(temp_path.to_path_buf()).unwrap();
    builder
      .add_file(
        temp_path.join("a.txt").as_path(),
        "0123456789".to_string().into_bytes(),
      )
      .unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);
    let virtual_fs = Arc::new(virtual_fs);
    let file = virtual_fs.open_file(&dest_path.join("a.txt")).unwrap();
    file.clone().seek_sync(SeekFrom::Current(2)).unwrap();
    let mut buf = vec![0; 2];
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"45");
    file.clone().seek_sync(SeekFrom::Current(-4)).unwrap();
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.clone().seek_sync(SeekFrom::Start(2)).unwrap();
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.clone().seek_sync(SeekFrom::End(2)).unwrap();
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"89");
    file.clone().seek_sync(SeekFrom::Current(-8)).unwrap();
    file.clone().read_sync(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    assert_eq!(
      file
        .clone()
        .seek_sync(SeekFrom::Current(-5))
        .err()
        .unwrap()
        .into_io_error()
        .to_string(),
      "An attempt was made to move the file pointer before the beginning of the file."
    );
    // go beyond the file length, then back
    file.clone().seek_sync(SeekFrom::Current(40)).unwrap();
    file.clone().seek_sync(SeekFrom::Current(-38)).unwrap();
    let read_buf = file.clone().read(2).await.unwrap();
    assert_eq!(read_buf.to_vec(), b"67");
    file.clone().seek_sync(SeekFrom::Current(-2)).unwrap();

    // read to the end of the file
    let all_buf = file.clone().read_all_sync().unwrap();
    assert_eq!(all_buf.to_vec(), b"6789");
    file.clone().seek_sync(SeekFrom::Current(-9)).unwrap();

    // try try_clone_inner and read_all_async
    let all_buf = file
      .try_clone_inner()
      .unwrap()
      .read_all_async()
      .await
      .unwrap();
    assert_eq!(all_buf.to_vec(), b"123456789");
  }
}
