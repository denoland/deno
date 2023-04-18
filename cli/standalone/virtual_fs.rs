use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_runtime::deno_fs::FsStat;
use serde::Deserialize;
use serde::Serialize;

use crate::util;

pub struct VirtualFsBuilder {
  root_path: PathBuf,
  root_dir: VirtualDirectory,
  files: Vec<Vec<u8>>,
  current_offset: u64,
  file_offsets: HashMap<String, u64>,
}

impl VirtualFsBuilder {
  pub fn new(root_path: PathBuf) -> Self {
    Self {
      root_dir: VirtualDirectory {
        name: root_path
          .file_stem()
          .unwrap()
          .to_string_lossy()
          .into_owned(),
        entries: Vec::new(),
      },
      root_path,
      files: Vec::new(),
      current_offset: 0,
      file_offsets: Default::default(),
    }
  }

  pub fn build_from_path(root_path: PathBuf) -> Result<Self, AnyError> {
    fn include_dir_recursive(
      builder: &mut VirtualFsBuilder,
      path: &Path,
    ) -> Result<(), AnyError> {
      let read_dir = std::fs::read_dir(path)
        .with_context(|| format!("Reading {}", path.display()))?;

      for entry in read_dir {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();

        if file_type.is_dir() {
          builder.add_dir(&path);
          include_dir_recursive(builder, &path)?;
        } else if file_type.is_file() {
          let file_bytes = std::fs::read(&path)
            .with_context(|| format!("Reading {}", path.display()))?;
          builder.add_file(&path, file_bytes);
        } else if file_type.is_symlink() {
          let target = std::fs::read_link(&path)
            .with_context(|| format!("Reading symlink {}", path.display()))?;
          builder.add_symlink(&path, &target);
        }
      }

      Ok(())
    }

    let mut builder = Self::new(root_path.clone());
    include_dir_recursive(&mut builder, &root_path)?;
    Ok(builder)
  }

  pub fn add_dir(&mut self, path: &Path) -> &mut VirtualDirectory {
    let path = path.strip_prefix(&self.root_path).unwrap();
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
            VirtualFsEntry::Dir(VirtualDirectory {
              name: name.to_string(),
              entries: Vec::new(),
            }),
          );
          insert_index
        }
      };
      match &mut current_dir.entries[index] {
        VirtualFsEntry::Dir(dir) => {
          current_dir = dir;
        }
        _ => unreachable!(),
      };
    }

    current_dir
  }

  pub fn add_file(&mut self, path: &Path, data: Vec<u8>) {
    let checksum = util::checksum::gen(&[&data]);
    let offset = if let Some(offset) = self.file_offsets.get(&checksum) {
      // duplicate file, reuse an old offset
      *offset
    } else {
      self.file_offsets.insert(checksum, self.current_offset);
      self.current_offset
    };

    let dir = self.add_dir(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    let data_len = data.len();
    match dir.entries.binary_search_by(|e| e.name().cmp(&name)) {
      Ok(_) => unreachable!(),
      Err(insert_index) => {
        dir.entries.insert(
          insert_index,
          VirtualFsEntry::File(VirtualFile {
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
  }

  pub fn add_symlink(&mut self, path: &Path, target: &Path) {
    let dest = target.strip_prefix(&self.root_path).unwrap().to_path_buf();
    let dir = self.add_dir(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    match dir.entries.binary_search_by(|e| e.name().cmp(&name)) {
      Ok(_) => unreachable!(),
      Err(insert_index) => {
        dir.entries.insert(
          insert_index,
          VirtualFsEntry::Symlink(VirtualSymlink {
            name: name.to_string(),
            dest_parts: dest
              .components()
              .map(|c| c.as_os_str().to_string_lossy().to_string())
              .collect::<Vec<_>>(),
          }),
        );
      }
    }
  }

  pub fn write_files(&self, writer: &mut impl Write) -> std::io::Result<()> {
    for file in &self.files {
      writer.write_all(file)?;
    }
    Ok(())
  }

  pub fn into_root_dir(self) -> VirtualDirectory {
    self.root_dir
  }
}

enum VirtualFsEntryRef<'a> {
  Dir(&'a VirtualDirectory),
  File(&'a VirtualFile),
  Symlink(&'a VirtualSymlink),
}

impl<'a> VirtualFsEntryRef<'a> {
  pub fn name(&self) -> &str {
    match self {
      VirtualFsEntryRef::Dir(dir) => &dir.name,
      VirtualFsEntryRef::File(file) => &file.name,
      VirtualFsEntryRef::Symlink(symlink) => &symlink.name,
    }
  }

  pub fn as_fs_state(&self) -> FsStat {
    match self {
      VirtualFsEntryRef::Dir(_) => FsStat {
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
      },
      VirtualFsEntryRef::File(file) => FsStat {
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
      },
      VirtualFsEntryRef::Symlink(_) => FsStat {
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
      },
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VirtualFsEntry {
  Dir(VirtualDirectory),
  File(VirtualFile),
  Symlink(VirtualSymlink),
}

impl VirtualFsEntry {
  pub fn name(&self) -> &str {
    match self {
      VirtualFsEntry::Dir(dir) => &dir.name,
      VirtualFsEntry::File(file) => &file.name,
      VirtualFsEntry::Symlink(symlink) => &symlink.name,
    }
  }

  fn as_ref(&self) -> VirtualFsEntryRef {
    match self {
      VirtualFsEntry::Dir(dir) => VirtualFsEntryRef::Dir(dir),
      VirtualFsEntry::File(file) => VirtualFsEntryRef::File(file),
      VirtualFsEntry::Symlink(symlink) => VirtualFsEntryRef::Symlink(symlink),
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualDirectory {
  pub name: String,
  // should be sorted by name
  pub entries: Vec<VirtualFsEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
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

pub struct VirtualFsRoot {
  pub dir: VirtualDirectory,
  pub root: PathBuf,
  pub start_file_offset: u64,
}

impl VirtualFsRoot {
  fn find_entry<'file>(
    &'file self,
    path: &Path,
  ) -> std::io::Result<(PathBuf, VirtualFsEntryRef<'file>)> {
    self.find_entry_inner(path, &mut HashSet::new())
  }

  fn find_entry_inner<'file>(
    &'file self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
  ) -> std::io::Result<(PathBuf, VirtualFsEntryRef<'file>)> {
    let mut path = Cow::Borrowed(path);
    loop {
      let (resolved_path, entry) =
        self.find_entry_no_follow_inner(&path, seen)?;
      match entry {
        VirtualFsEntryRef::Symlink(symlink) => {
          if !seen.insert(path.to_path_buf()) {
            return Err(std::io::Error::new(
              std::io::ErrorKind::Other,
              "circular symlinks",
            ));
          }
          path = Cow::Owned(symlink.resolve_dest_from_root(&self.root));
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
  ) -> std::io::Result<(PathBuf, VirtualFsEntryRef)> {
    self.find_entry_no_follow_inner(path, &mut HashSet::new())
  }

  fn find_entry_no_follow_inner<'file>(
    &'file self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
  ) -> std::io::Result<(PathBuf, VirtualFsEntryRef<'file>)> {
    eprintln!("PATH: {:?}", path.as_os_str().to_string_lossy());
    let relative_path = match path.strip_prefix(&self.root) {
      Ok(p) => p,
      Err(_) => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "path not found",
        ));
      }
    };
    let mut final_path = self.root.clone();
    let mut current_entry = VirtualFsEntryRef::Dir(&self.dir);
    for component in relative_path.components() {
      let component = component.as_os_str().to_string_lossy();
      let current_dir = match current_entry {
        VirtualFsEntryRef::Dir(dir) => {
          final_path.push(component.as_ref());
          dir
        }
        VirtualFsEntryRef::Symlink(symlink) => {
          let dest = symlink.resolve_dest_from_root(&self.root);
          let (resolved_path, entry) = self.find_entry_inner(&dest, seen)?;
          final_path = resolved_path; // overwrite with the new resolved path
          match entry {
            VirtualFsEntryRef::Dir(dir) => {
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

pub struct FileBackedVirtualFs {
  file: File,
  fs_root: VirtualFsRoot,
}

impl FileBackedVirtualFs {
  pub fn new(file: File, fs_root: VirtualFsRoot) -> Self {
    Self { file, fs_root }
  }

  pub fn symlink_metadata(&self, path: &Path) -> std::io::Result<FsStat> {
    let (_, entry) = self.fs_root.find_entry_no_follow(path)?;
    Ok(entry.as_fs_state())
  }

  pub fn metadata(&self, path: &Path) -> std::io::Result<FsStat> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    Ok(entry.as_fs_state())
  }

  pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (path, _) = self.fs_root.find_entry(path)?;
    Ok(path.to_path_buf())
  }

  pub fn read_to_string(&mut self, path: &Path) -> std::io::Result<String> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    let file = match entry {
      VirtualFsEntryRef::Dir(_) => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::Other,
          "path is a directory",
        ));
      }
      VirtualFsEntryRef::Symlink(_) => unreachable!(),
      VirtualFsEntryRef::File(file) => file,
    };
    self.file.seek(SeekFrom::Start(
      self.fs_root.start_file_offset + file.offset,
    ))?;
    let mut buf = vec![0; file.len as usize];
    self.file.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|_| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "stream did not contain valid UTF-8",
      )
    })
  }
}

#[cfg(test)]
mod test {
  use test_util::TempDir;

  use super::*;

  // todo(THIS PR): tests for circular symlinks

  #[test]
  fn builds_and_uses_virtual_fs() {
    let temp_dir = TempDir::new();
    let src_path = temp_dir.path().join("src");
    let mut builder = VirtualFsBuilder::new(src_path.clone());
    builder.add_file(&src_path.join("a.txt"), "data".into());
    builder.add_file(&src_path.join("b.txt"), "data".into());
    assert_eq!(builder.files.len(), 1); // because duplicate data
    builder.add_file(&src_path.join("c.txt"), "c".into());
    builder.add_file(&src_path.join("sub_dir").join("d.txt"), "d".into());
    builder.add_file(&src_path.join("e.txt"), "e".into());
    builder.add_symlink(
      &src_path.join("sub_dir").join("e.txt"),
      &src_path.join("e.txt"),
    );

    // get the virtual fs
    let (dest_path, mut virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(
      virtual_fs.read_to_string(&dest_path.join("a.txt")).unwrap(),
      "data",
    );
    assert_eq!(
      virtual_fs.read_to_string(&dest_path.join("b.txt")).unwrap(),
      "data",
    );

    // attempt reading a symlink
    assert_eq!(
      virtual_fs
        .read_to_string(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap(),
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
        .symlink_metadata(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .is_symlink
    );
    assert!(
      virtual_fs
        .metadata(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .is_file
    );
    assert!(
      virtual_fs
        .metadata(&dest_path.join("sub_dir"))
        .unwrap()
        .is_directory,
    );
    assert!(
      virtual_fs
        .metadata(&dest_path.join("e.txt"))
        .unwrap()
        .is_file,
    );
  }

  #[test]
  fn test_build_from_path() {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all("src/nested/sub_dir");
    temp_dir.write("src/a.txt", "data");
    temp_dir.write("src/b.txt", "data");
    util::fs::symlink_dir(
      &temp_dir.path().join("src/nested/sub_dir"),
      &temp_dir.path().join("src/sub_dir_link"),
    )
    .unwrap();
    temp_dir.write("src/nested/sub_dir/c.txt", "c");

    // build and create the virtual fs
    let builder =
      VirtualFsBuilder::build_from_path(temp_dir.path().join("src")).unwrap();
    let (dest_path, mut virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(
      virtual_fs.read_to_string(&dest_path.join("a.txt")).unwrap(),
      "data",
    );
    assert_eq!(
      virtual_fs.read_to_string(&dest_path.join("b.txt")).unwrap(),
      "data",
    );

    assert_eq!(
      virtual_fs
        .read_to_string(&dest_path.join("nested").join("sub_dir").join("c.txt"))
        .unwrap(),
      "c",
    );
    assert_eq!(
      virtual_fs
        .read_to_string(&dest_path.join("sub_dir_link").join("c.txt"))
        .unwrap(),
      "c",
    );
    assert!(
      virtual_fs
        .symlink_metadata(&dest_path.join("sub_dir_link"))
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
    builder: VirtualFsBuilder,
    temp_dir: &TempDir,
  ) -> (PathBuf, FileBackedVirtualFs) {
    let virtual_fs_file = temp_dir.path().join("virtual_fs");
    {
      let mut file = std::fs::File::create(&virtual_fs_file).unwrap();
      builder.write_files(&mut file).unwrap();
    }
    let root_dir = builder.into_root_dir();
    let file = std::fs::File::open(&virtual_fs_file).unwrap();
    let dest_path = temp_dir.path().join("dest");
    (
      dest_path.clone(),
      FileBackedVirtualFs::new(
        file,
        VirtualFsRoot {
          dir: root_dir,
          root: dest_path,
          start_file_offset: 0,
        },
      ),
    )
  }
}
