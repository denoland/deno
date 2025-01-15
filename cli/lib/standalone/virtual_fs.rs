// Copyright 2018-2025 the Deno authors. MIT license.

use std::cmp::Ordering;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Copy, Clone)]
pub enum VfsFileSubDataKind {
  /// Raw bytes of the file.
  Raw,
  /// Bytes to use for module loading. For example, for TypeScript
  /// files this will be the transpiled JavaScript source.
  ModuleGraph,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WindowsSystemRootablePath {
  /// The root of the system above any drive letters.
  WindowSystemRoot,
  Path(PathBuf),
}

impl WindowsSystemRootablePath {
  pub fn join(&self, name_component: &str) -> PathBuf {
    // this method doesn't handle multiple components
    debug_assert!(
      !name_component.contains('\\'),
      "Invalid component: {}",
      name_component
    );
    debug_assert!(
      !name_component.contains('/'),
      "Invalid component: {}",
      name_component
    );

    match self {
      WindowsSystemRootablePath::WindowSystemRoot => {
        // windows drive letter
        PathBuf::from(&format!("{}\\", name_component))
      }
      WindowsSystemRootablePath::Path(path) => path.join(name_component),
    }
  }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum FileSystemCaseSensitivity {
  #[serde(rename = "s")]
  Sensitive,
  #[serde(rename = "i")]
  Insensitive,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VirtualDirectoryEntries(Vec<VfsEntry>);

impl VirtualDirectoryEntries {
  pub fn new(mut entries: Vec<VfsEntry>) -> Self {
    // needs to be sorted by name
    entries.sort_by(|a, b| a.name().cmp(b.name()));
    Self(entries)
  }

  pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, VfsEntry> {
    self.0.iter_mut()
  }

  pub fn iter(&self) -> std::slice::Iter<'_, VfsEntry> {
    self.0.iter()
  }

  pub fn take_inner(&mut self) -> Vec<VfsEntry> {
    std::mem::take(&mut self.0)
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn get_by_name(
    &self,
    name: &str,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> Option<&VfsEntry> {
    self
      .binary_search(name, case_sensitivity)
      .ok()
      .map(|index| &self.0[index])
  }

  pub fn get_mut_by_name(
    &mut self,
    name: &str,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> Option<&mut VfsEntry> {
    self
      .binary_search(name, case_sensitivity)
      .ok()
      .map(|index| &mut self.0[index])
  }

  pub fn get_mut_by_index(&mut self, index: usize) -> Option<&mut VfsEntry> {
    self.0.get_mut(index)
  }

  pub fn binary_search(
    &self,
    name: &str,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> Result<usize, usize> {
    match case_sensitivity {
      FileSystemCaseSensitivity::Sensitive => {
        self.0.binary_search_by(|e| e.name().cmp(name))
      }
      FileSystemCaseSensitivity::Insensitive => self.0.binary_search_by(|e| {
        e.name()
          .chars()
          .zip(name.chars())
          .map(|(a, b)| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()))
          .find(|&ord| ord != Ordering::Equal)
          .unwrap_or_else(|| e.name().len().cmp(&name.len()))
      }),
    }
  }

  pub fn insert(
    &mut self,
    entry: VfsEntry,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> usize {
    match self.binary_search(entry.name(), case_sensitivity) {
      Ok(index) => {
        self.0[index] = entry;
        index
      }
      Err(insert_index) => {
        self.0.insert(insert_index, entry);
        insert_index
      }
    }
  }

  pub fn insert_or_modify(
    &mut self,
    name: &str,
    case_sensitivity: FileSystemCaseSensitivity,
    on_insert: impl FnOnce() -> VfsEntry,
    on_modify: impl FnOnce(&mut VfsEntry),
  ) -> usize {
    match self.binary_search(name, case_sensitivity) {
      Ok(index) => {
        on_modify(&mut self.0[index]);
        index
      }
      Err(insert_index) => {
        self.0.insert(insert_index, on_insert());
        insert_index
      }
    }
  }

  pub fn remove(&mut self, index: usize) -> VfsEntry {
    self.0.remove(index)
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualDirectory {
  #[serde(rename = "n")]
  pub name: String,
  // should be sorted by name
  #[serde(rename = "e")]
  pub entries: VirtualDirectoryEntries,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OffsetWithLength {
  #[serde(rename = "o")]
  pub offset: u64,
  #[serde(rename = "l")]
  pub len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFile {
  #[serde(rename = "n")]
  pub name: String,
  #[serde(rename = "o")]
  pub offset: OffsetWithLength,
  /// Offset file to use for module loading when it differs from the
  /// raw file. Often this will be the same offset as above for data
  /// such as JavaScript files, but for TypeScript files the `offset`
  /// will be the original raw bytes when included as an asset and this
  /// offset will be to the transpiled JavaScript source.
  #[serde(rename = "m")]
  pub module_graph_offset: OffsetWithLength,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualSymlinkParts(Vec<String>);

impl VirtualSymlinkParts {
  pub fn from_path(path: &Path) -> Self {
    Self(
      path
        .components()
        .filter(|c| !matches!(c, std::path::Component::RootDir))
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect(),
    )
  }

  pub fn take_parts(&mut self) -> Vec<String> {
    std::mem::take(&mut self.0)
  }

  pub fn parts(&self) -> &[String] {
    &self.0
  }

  pub fn set_parts(&mut self, parts: Vec<String>) {
    self.0 = parts;
  }

  pub fn display(&self) -> String {
    self.0.join("/")
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualSymlink {
  #[serde(rename = "n")]
  pub name: String,
  #[serde(rename = "p")]
  pub dest_parts: VirtualSymlinkParts,
}

impl VirtualSymlink {
  pub fn resolve_dest_from_root(&self, root: &Path) -> PathBuf {
    let mut dest = root.to_path_buf();
    for part in &self.dest_parts.0 {
      dest.push(part);
    }
    dest
  }
}

#[derive(Debug, Copy, Clone)]
pub enum VfsEntryRef<'a> {
  Dir(&'a VirtualDirectory),
  File(&'a VirtualFile),
  Symlink(&'a VirtualSymlink),
}

impl VfsEntryRef<'_> {
  pub fn name(&self) -> &str {
    match self {
      Self::Dir(dir) => &dir.name,
      Self::File(file) => &file.name,
      Self::Symlink(symlink) => &symlink.name,
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
      Self::Dir(dir) => &dir.name,
      Self::File(file) => &file.name,
      Self::Symlink(symlink) => &symlink.name,
    }
  }

  pub fn as_ref(&self) -> VfsEntryRef {
    match self {
      VfsEntry::Dir(dir) => VfsEntryRef::Dir(dir),
      VfsEntry::File(file) => VfsEntryRef::File(file),
      VfsEntry::Symlink(symlink) => VfsEntryRef::Symlink(symlink),
    }
  }
}
