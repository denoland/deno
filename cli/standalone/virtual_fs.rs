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

use deno_runtime::deno_node::NodeFsMetadata;
use deno_runtime::deno_node::PathClean;
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
            dest: dest
              .components()
              .map(|c| c.as_os_str().to_string_lossy())
              .collect::<Vec<_>>()
              .join("/"), // use the same separator regardless of platform
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

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct VirtualDirectory {
  pub name: String,
  // should be sorted by name
  pub entries: Vec<VirtualFsEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct VirtualFile {
  pub name: String,
  pub offset: u64,
  pub len: u64,
}

#[derive(Serialize, Deserialize)]
pub struct VirtualSymlink {
  pub name: String,
  pub dest: String,
}

pub struct VirtualFsRoot {
  pub dir: VirtualDirectory,
  pub root: PathBuf,
  pub start_file_offset: u64,
}

impl VirtualFsRoot {
  fn find_entry<'path, 'file>(
    &'file self,
    path: &'path Path,
  ) -> std::io::Result<(Cow<'path, Path>, VirtualFsEntryRef<'file>)> {
    let mut found = HashSet::with_capacity(6);
    let mut path = Cow::Borrowed(path);
    loop {
      let entry = self.find_entry_no_follow(&path)?;
      match entry {
        VirtualFsEntryRef::Symlink(symlink) => {
          if !found.insert(path) {
            return Err(std::io::Error::new(
              std::io::ErrorKind::Other,
              "circular symlinks",
            ));
          } else if found.len() > 5 {
            return Err(std::io::Error::new(
              std::io::ErrorKind::Other,
              "too many symlinks",
            ));
          }
          path = Cow::Owned(self.root.join(&symlink.dest).clean());
        }
        _ => {
          return Ok((path, entry));
        }
      }
    }
  }

  fn find_entry_no_follow(
    &self,
    path: &Path,
  ) -> std::io::Result<VirtualFsEntryRef> {
    let mut entries = &self.dir.entries;
    let path = match path.strip_prefix(&self.root) {
      Ok(p) => p,
      Err(_) => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "path not found",
        ));
      }
    };
    let mut current_entry = VirtualFsEntryRef::Dir(&self.dir);
    for component in path.components() {
      let component = component.as_os_str().to_string_lossy();
      let current_dir = match current_entry {
        VirtualFsEntryRef::Dir(dir) => dir,
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

    Ok(current_entry)
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

  pub fn metadata(&self, path: &Path) -> std::io::Result<NodeFsMetadata> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    Ok(match entry {
      VirtualFsEntryRef::Dir(dir) => NodeFsMetadata {
        is_dir: true,
        is_file: false,
      },
      VirtualFsEntryRef::File(file) => NodeFsMetadata {
        is_dir: false,
        is_file: true,
      },
      VirtualFsEntryRef::Symlink(_) => {
        unreachable!();
      }
    })
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

    // write out the file data to a file
    let virtual_fs_file = temp_dir.path().join("virtual_fs");
    {
      let mut file = std::fs::File::create(&virtual_fs_file).unwrap();
      builder.write_files(&mut file).unwrap();
    }

    // get the virtual fs
    let root_dir = builder.into_root_dir();

    let file = std::fs::File::open(&virtual_fs_file).unwrap();
    let dest_path = temp_dir.path().join("dest"); // test out using a separate directory
    let mut virtual_fs = FileBackedVirtualFs::new(
      file,
      VirtualFsRoot {
        dir: root_dir,
        root: dest_path.clone(),
        start_file_offset: 0,
      },
    );

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
    assert_eq!(
      virtual_fs
        .metadata(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap(),
      NodeFsMetadata {
        is_dir: false,
        is_file: true,
      },
    );
    assert_eq!(
      virtual_fs.metadata(&dest_path.join("sub_dir")).unwrap(),
      NodeFsMetadata {
        is_dir: true,
        is_file: false,
      },
    );
  }
}
