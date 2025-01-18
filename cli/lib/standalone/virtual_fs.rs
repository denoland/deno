// Copyright 2018-2025 the Deno authors. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_path_util::normalize_path;
use deno_path_util::strip_unc_prefix;
use deno_runtime::colors;
use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::anyhow::Context;
use deno_runtime::deno_core::error::AnyError;
use indexmap::IndexSet;
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

pub static DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME: &str =
  ".deno_compile_node_modules";

#[derive(Debug)]
pub struct BuiltVfs {
  pub root_path: WindowsSystemRootablePath,
  pub case_sensitivity: FileSystemCaseSensitivity,
  pub entries: VirtualDirectoryEntries,
  pub files: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct VfsBuilder {
  executable_root: VirtualDirectory,
  files: Vec<Vec<u8>>,
  current_offset: u64,
  file_offsets: HashMap<String, u64>,
  /// The minimum root directory that should be included in the VFS.
  min_root_dir: Option<WindowsSystemRootablePath>,
  case_sensitivity: FileSystemCaseSensitivity,
}

impl Default for VfsBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl VfsBuilder {
  pub fn new() -> Self {
    Self {
      executable_root: VirtualDirectory {
        name: "/".to_string(),
        entries: Default::default(),
      },
      files: Vec::new(),
      current_offset: 0,
      file_offsets: Default::default(),
      min_root_dir: Default::default(),
      // This is not exactly correct because file systems on these OSes
      // may be case-sensitive or not based on the directory, but this
      // is a good enough approximation and limitation. In the future,
      // we may want to store this information per directory instead
      // depending on the feedback we get.
      case_sensitivity: if cfg!(windows) || cfg!(target_os = "macos") {
        FileSystemCaseSensitivity::Insensitive
      } else {
        FileSystemCaseSensitivity::Sensitive
      },
    }
  }

  pub fn case_sensitivity(&self) -> FileSystemCaseSensitivity {
    self.case_sensitivity
  }

  pub fn files_len(&self) -> usize {
    self.files.len()
  }

  /// Add a directory that might be the minimum root directory
  /// of the VFS.
  ///
  /// For example, say the user has a deno.json and specifies an
  /// import map in a parent directory. The import map won't be
  /// included in the VFS, but its base will meaning we need to
  /// tell the VFS builder to include the base of the import map
  /// by calling this method.
  pub fn add_possible_min_root_dir(&mut self, path: &Path) {
    self.add_dir_raw(path);

    match &self.min_root_dir {
      Some(WindowsSystemRootablePath::WindowSystemRoot) => {
        // already the root dir
      }
      Some(WindowsSystemRootablePath::Path(current_path)) => {
        let mut common_components = Vec::new();
        for (a, b) in current_path.components().zip(path.components()) {
          if a != b {
            break;
          }
          common_components.push(a);
        }
        if common_components.is_empty() {
          if cfg!(windows) {
            self.min_root_dir =
              Some(WindowsSystemRootablePath::WindowSystemRoot);
          } else {
            self.min_root_dir =
              Some(WindowsSystemRootablePath::Path(PathBuf::from("/")));
          }
        } else {
          self.min_root_dir = Some(WindowsSystemRootablePath::Path(
            common_components.iter().collect(),
          ));
        }
      }
      None => {
        self.min_root_dir =
          Some(WindowsSystemRootablePath::Path(path.to_path_buf()));
      }
    }
  }

  pub fn add_dir_recursive(&mut self, path: &Path) -> Result<(), AnyError> {
    let target_path = self.resolve_target_path(path)?;
    self.add_dir_recursive_not_symlink(&target_path)
  }

  fn add_dir_recursive_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    self.add_dir_raw(path);
    // ok, building fs implementation
    #[allow(clippy::disallowed_methods)]
    let read_dir = std::fs::read_dir(path)
      .with_context(|| format!("Reading {}", path.display()))?;

    let mut dir_entries =
      read_dir.into_iter().collect::<Result<Vec<_>, _>>()?;
    dir_entries.sort_by_cached_key(|entry| entry.file_name()); // determinism

    for entry in dir_entries {
      let file_type = entry.file_type()?;
      let path = entry.path();

      if file_type.is_dir() {
        self.add_dir_recursive_not_symlink(&path)?;
      } else if file_type.is_file() {
        self.add_file_at_path_not_symlink(&path)?;
      } else if file_type.is_symlink() {
        match self.add_symlink(&path) {
          Ok(target) => match target {
            SymlinkTarget::File(target) => {
              self.add_file_at_path_not_symlink(&target)?
            }
            SymlinkTarget::Dir(target) => {
              self.add_dir_recursive_not_symlink(&target)?;
            }
          },
          Err(err) => {
            log::warn!(
            "{} Failed resolving symlink. Ignoring.\n    Path: {}\n    Message: {:#}",
            colors::yellow("Warning"),
            path.display(),
            err
          );
          }
        }
      }
    }

    Ok(())
  }

  fn add_dir_raw(&mut self, path: &Path) -> &mut VirtualDirectory {
    log::debug!("Ensuring directory '{}'", path.display());
    debug_assert!(path.is_absolute());
    let mut current_dir = &mut self.executable_root;

    for component in path.components() {
      if matches!(component, std::path::Component::RootDir) {
        continue;
      }
      let name = component.as_os_str().to_string_lossy();
      let index = current_dir.entries.insert_or_modify(
        &name,
        self.case_sensitivity,
        || {
          VfsEntry::Dir(VirtualDirectory {
            name: name.to_string(),
            entries: Default::default(),
          })
        },
        |_| {
          // ignore
        },
      );
      match current_dir.entries.get_mut_by_index(index) {
        Some(VfsEntry::Dir(dir)) => {
          current_dir = dir;
        }
        _ => unreachable!(),
      };
    }

    current_dir
  }

  pub fn get_system_root_dir_mut(&mut self) -> &mut VirtualDirectory {
    &mut self.executable_root
  }

  pub fn get_dir_mut(&mut self, path: &Path) -> Option<&mut VirtualDirectory> {
    debug_assert!(path.is_absolute());
    let mut current_dir = &mut self.executable_root;

    for component in path.components() {
      if matches!(component, std::path::Component::RootDir) {
        continue;
      }
      let name = component.as_os_str().to_string_lossy();
      let entry = current_dir
        .entries
        .get_mut_by_name(&name, self.case_sensitivity)?;
      match entry {
        VfsEntry::Dir(dir) => {
          current_dir = dir;
        }
        _ => unreachable!(),
      };
    }

    Some(current_dir)
  }

  pub fn add_file_at_path(&mut self, path: &Path) -> Result<(), AnyError> {
    // ok, building fs implementation
    #[allow(clippy::disallowed_methods)]
    let file_bytes = std::fs::read(path)
      .with_context(|| format!("Reading {}", path.display()))?;
    self.add_file_with_data(path, file_bytes, VfsFileSubDataKind::Raw)
  }

  fn add_file_at_path_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    // ok, building fs implementation
    #[allow(clippy::disallowed_methods)]
    let file_bytes = std::fs::read(path)
      .with_context(|| format!("Reading {}", path.display()))?;
    self.add_file_with_data_raw(path, file_bytes, VfsFileSubDataKind::Raw)
  }

  pub fn add_file_with_data(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    sub_data_kind: VfsFileSubDataKind,
  ) -> Result<(), AnyError> {
    // ok, fs implementation
    #[allow(clippy::disallowed_methods)]
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
      format!("Resolving target path for '{}'", path.display())
    })?;
    if metadata.is_symlink() {
      let target = self.add_symlink(path)?.into_path_buf();
      self.add_file_with_data_raw(&target, data, sub_data_kind)
    } else {
      self.add_file_with_data_raw(path, data, sub_data_kind)
    }
  }

  pub fn add_file_with_data_raw(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    sub_data_kind: VfsFileSubDataKind,
  ) -> Result<(), AnyError> {
    log::debug!("Adding file '{}'", path.display());
    let checksum = crate::util::checksum::gen(&[&data]);
    let case_sensitivity = self.case_sensitivity;
    let offset = if let Some(offset) = self.file_offsets.get(&checksum) {
      // duplicate file, reuse an old offset
      *offset
    } else {
      self.file_offsets.insert(checksum, self.current_offset);
      self.current_offset
    };

    let dir = self.add_dir_raw(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    let offset_and_len = OffsetWithLength {
      offset,
      len: data.len() as u64,
    };
    dir.entries.insert_or_modify(
      &name,
      case_sensitivity,
      || {
        VfsEntry::File(VirtualFile {
          name: name.to_string(),
          offset: offset_and_len,
          module_graph_offset: offset_and_len,
        })
      },
      |entry| match entry {
        VfsEntry::File(virtual_file) => match sub_data_kind {
          VfsFileSubDataKind::Raw => {
            virtual_file.offset = offset_and_len;
          }
          VfsFileSubDataKind::ModuleGraph => {
            virtual_file.module_graph_offset = offset_and_len;
          }
        },
        VfsEntry::Dir(_) | VfsEntry::Symlink(_) => unreachable!(),
      },
    );

    // new file, update the list of files
    if self.current_offset == offset {
      self.files.push(data);
      self.current_offset += offset_and_len.len;
    }

    Ok(())
  }

  fn resolve_target_path(&mut self, path: &Path) -> Result<PathBuf, AnyError> {
    // ok, fs implementation
    #[allow(clippy::disallowed_methods)]
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
      format!("Resolving target path for '{}'", path.display())
    })?;
    if metadata.is_symlink() {
      Ok(self.add_symlink(path)?.into_path_buf())
    } else {
      Ok(path.to_path_buf())
    }
  }

  pub fn add_symlink(
    &mut self,
    path: &Path,
  ) -> Result<SymlinkTarget, AnyError> {
    self.add_symlink_inner(path, &mut IndexSet::new())
  }

  fn add_symlink_inner(
    &mut self,
    path: &Path,
    visited: &mut IndexSet<PathBuf>,
  ) -> Result<SymlinkTarget, AnyError> {
    log::debug!("Adding symlink '{}'", path.display());
    let target = strip_unc_prefix(
      // ok, fs implementation
      #[allow(clippy::disallowed_methods)]
      std::fs::read_link(path)
        .with_context(|| format!("Reading symlink '{}'", path.display()))?,
    );
    let case_sensitivity = self.case_sensitivity;
    let target = normalize_path(path.parent().unwrap().join(&target));
    let dir = self.add_dir_raw(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    dir.entries.insert_or_modify(
      &name,
      case_sensitivity,
      || {
        VfsEntry::Symlink(VirtualSymlink {
          name: name.to_string(),
          dest_parts: VirtualSymlinkParts::from_path(&target),
        })
      },
      |_| {
        // ignore previously inserted
      },
    );
    // ok, fs implementation
    #[allow(clippy::disallowed_methods)]
    let target_metadata =
      std::fs::symlink_metadata(&target).with_context(|| {
        format!("Reading symlink target '{}'", target.display())
      })?;
    if target_metadata.is_symlink() {
      if !visited.insert(target.clone()) {
        // todo: probably don't error in this scenario
        bail!(
          "Circular symlink detected: {} -> {}",
          visited
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" -> "),
          target.display()
        );
      }
      self.add_symlink_inner(&target, visited)
    } else if target_metadata.is_dir() {
      Ok(SymlinkTarget::Dir(target))
    } else {
      Ok(SymlinkTarget::File(target))
    }
  }

  pub fn build(self) -> BuiltVfs {
    fn strip_prefix_from_symlinks(
      dir: &mut VirtualDirectory,
      parts: &[String],
    ) {
      for entry in dir.entries.iter_mut() {
        match entry {
          VfsEntry::Dir(dir) => {
            strip_prefix_from_symlinks(dir, parts);
          }
          VfsEntry::File(_) => {}
          VfsEntry::Symlink(symlink) => {
            let parts = symlink
              .dest_parts
              .take_parts()
              .into_iter()
              .skip(parts.len())
              .collect();
            symlink.dest_parts.set_parts(parts);
          }
        }
      }
    }

    let mut current_dir = self.executable_root;
    let mut current_path = if cfg!(windows) {
      WindowsSystemRootablePath::WindowSystemRoot
    } else {
      WindowsSystemRootablePath::Path(PathBuf::from("/"))
    };
    loop {
      if current_dir.entries.len() != 1 {
        break;
      }
      if self.min_root_dir.as_ref() == Some(&current_path) {
        break;
      }
      match current_dir.entries.iter().next().unwrap() {
        VfsEntry::Dir(dir) => {
          if dir.name == DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME {
            // special directory we want to maintain
            break;
          }
          match current_dir.entries.remove(0) {
            VfsEntry::Dir(dir) => {
              current_path =
                WindowsSystemRootablePath::Path(current_path.join(&dir.name));
              current_dir = dir;
            }
            _ => unreachable!(),
          };
        }
        VfsEntry::File(_) | VfsEntry::Symlink(_) => break,
      }
    }
    if let WindowsSystemRootablePath::Path(path) = &current_path {
      strip_prefix_from_symlinks(
        &mut current_dir,
        VirtualSymlinkParts::from_path(path).parts(),
      );
    }
    BuiltVfs {
      root_path: current_path,
      case_sensitivity: self.case_sensitivity,
      entries: current_dir.entries,
      files: self.files,
    }
  }
}

#[derive(Debug)]
pub enum SymlinkTarget {
  File(PathBuf),
  Dir(PathBuf),
}

impl SymlinkTarget {
  pub fn into_path_buf(self) -> PathBuf {
    match self {
      Self::File(path) => path,
      Self::Dir(path) => path,
    }
  }
}
