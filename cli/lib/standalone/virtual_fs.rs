// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::fmt;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use deno_path_util::normalize_path;
use deno_path_util::strip_unc_prefix;
use deno_runtime::colors;
use deno_runtime::deno_core::anyhow::Context;
use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;
use indexmap::IndexSet;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de;
use serde::de::SeqAccess;
use serde::de::Visitor;

use crate::util::text_encoding::is_valid_utf8;

#[derive(Debug, PartialEq, Eq)]
pub enum WindowsSystemRootablePath {
  /// The root of the system above any drive letters.
  WindowSystemRoot,
  Path(PathBuf),
}

impl WindowsSystemRootablePath {
  pub fn root_for_current_os() -> Self {
    if cfg!(windows) {
      WindowsSystemRootablePath::WindowSystemRoot
    } else {
      WindowsSystemRootablePath::Path(PathBuf::from("/"))
    }
  }

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

impl FileSystemCaseSensitivity {
  pub fn cmp_name(&self, a: &str, b: &str) -> Ordering {
    match self {
      FileSystemCaseSensitivity::Sensitive => a.cmp(b),
      FileSystemCaseSensitivity::Insensitive => a
        .chars()
        .zip(b.chars())
        .map(|(a, b)| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()))
        .find(|&ord| ord != Ordering::Equal)
        .unwrap_or_else(|| a.len().cmp(&b.len())),
    }
  }

  pub fn eq_name(&self, a: &str, b: &str) -> bool {
    self.cmp_name(a, b) == Ordering::Equal
  }
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

  pub fn get_by_index(&self, index: usize) -> Option<&VfsEntry> {
    self.0.get(index)
  }

  pub fn binary_search(
    &self,
    name: &str,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> Result<usize, usize> {
    self
      .0
      .binary_search_by(|e| case_sensitivity.cmp_name(e.name(), name))
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

#[derive(Debug, Clone, Copy)]
pub struct OffsetWithLength {
  pub offset: u64,
  pub len: u64,
}

// serialize as an array in order to save space
impl Serialize for OffsetWithLength {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let array = [self.offset, self.len];
    array.serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for OffsetWithLength {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct OffsetWithLengthVisitor;

    impl<'de> Visitor<'de> for OffsetWithLengthVisitor {
      type Value = OffsetWithLength;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an array with two elements: [offset, len]")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: SeqAccess<'de>,
      {
        let offset = seq
          .next_element()?
          .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let len = seq
          .next_element()?
          .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        Ok(OffsetWithLength { offset, len })
      }
    }

    deserializer.deserialize_seq(OffsetWithLengthVisitor)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFile {
  #[serde(rename = "n")]
  pub name: String,
  #[serde(rename = "o")]
  pub offset: OffsetWithLength,
  #[serde(default, rename = "u", skip_serializing_if = "is_false")]
  pub is_valid_utf8: bool,
  #[serde(rename = "m", skip_serializing_if = "Option::is_none")]
  pub transpiled_offset: Option<OffsetWithLength>,
  #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
  pub cjs_export_analysis_offset: Option<OffsetWithLength>,
  #[serde(rename = "s", skip_serializing_if = "Option::is_none")]
  pub source_map_offset: Option<OffsetWithLength>,
  #[serde(rename = "t", skip_serializing_if = "Option::is_none")]
  pub mtime: Option<u128>, // mtime in milliseconds
  #[serde(default, rename = "x", skip_serializing_if = "is_false")]
  pub executable: bool,
}

fn is_false(value: &bool) -> bool {
  !value
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualSymlinkParts(Vec<String>);

impl VirtualSymlinkParts {
  pub fn from_path(path: &Path) -> Self {
    Self(
      path
        .components()
        .filter(|c| !matches!(c, std::path::Component::RootDir))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
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
  /// Whether the symlink target is a directory (needed on Windows where
  /// file and directory symlinks are distinct).
  #[serde(rename = "d", default, skip_serializing_if = "std::ops::Not::not")]
  pub dest_is_dir: bool,
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

  pub fn as_ref(&self) -> VfsEntryRef<'_> {
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

#[derive(Debug, Default)]
struct FilesData {
  files: Vec<Vec<u8>>,
  current_offset: u64,
  file_offsets: HashMap<(String, usize), OffsetWithLength>,
}

impl FilesData {
  pub fn file_bytes(&self, offset: OffsetWithLength) -> Option<&[u8]> {
    if offset.len == 0 {
      return Some(&[]);
    }

    // the debug assertions in this method should never happen
    // because it would indicate providing an offset not in the vfs
    let mut count: u64 = 0;
    for file in &self.files {
      // clippy wanted a match
      match count.cmp(&offset.offset) {
        Ordering::Equal => {
          debug_assert_eq!(offset.len, file.len() as u64);
          if offset.len == file.len() as u64 {
            return Some(file);
          } else {
            return None;
          }
        }
        Ordering::Less => {
          count += file.len() as u64;
        }
        Ordering::Greater => {
          debug_assert!(false);
          return None;
        }
      }
    }
    debug_assert!(false);
    None
  }

  pub fn add_data(&mut self, data: Vec<u8>) -> OffsetWithLength {
    if data.is_empty() {
      return OffsetWithLength { offset: 0, len: 0 };
    }
    let checksum = crate::util::checksum::r#gen(&[&data]);
    match self.file_offsets.entry((checksum, data.len())) {
      Entry::Occupied(occupied_entry) => {
        let offset_and_len = *occupied_entry.get();
        debug_assert_eq!(data.len() as u64, offset_and_len.len);
        offset_and_len
      }
      Entry::Vacant(vacant_entry) => {
        let offset_and_len = OffsetWithLength {
          offset: self.current_offset,
          len: data.len() as u64,
        };
        vacant_entry.insert(offset_and_len);
        self.current_offset += offset_and_len.len;
        self.files.push(data);
        offset_and_len
      }
    }
  }
}

pub struct AddFileDataOptions {
  pub data: Vec<u8>,
  pub mtime: Option<SystemTime>,
  pub maybe_transpiled: Option<Vec<u8>>,
  pub maybe_source_map: Option<Vec<u8>>,
  pub maybe_cjs_export_analysis: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct VfsBuilder {
  executable_root: VirtualDirectory,
  files: FilesData,
  /// The minimum root directory that should be included in the VFS.
  min_root_dir: Option<WindowsSystemRootablePath>,
  case_sensitivity: FileSystemCaseSensitivity,
  exclude_paths: HashSet<PathBuf>,
  /// Cache of path canonicalization results. Maps original paths to their
  /// canonical forms, used to detect aliased parent directories.
  /// - `None` = path is already canonical (no changes needed)
  /// - `Some(path)` = path was aliased, use this canonical form instead
  ///
  /// Note: `None` is also inserted upfront in `ensure_canonical_dir` as a
  /// sentinel to prevent infinite recursion on circular symlinks. It gets
  /// overwritten to `Some(...)` if the path turns out to have aliases.
  canonical_path_cache: HashMap<PathBuf, Option<PathBuf>>,
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
      files: Default::default(),
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
      exclude_paths: Default::default(),
      canonical_path_cache: Default::default(),
    }
  }

  pub fn case_sensitivity(&self) -> FileSystemCaseSensitivity {
    self.case_sensitivity
  }

  pub fn files_len(&self) -> usize {
    self.files.files.len()
  }

  pub fn file_bytes(&self, offset: OffsetWithLength) -> Option<&[u8]> {
    self.files.file_bytes(offset)
  }

  pub fn add_exclude_path(&mut self, path: PathBuf) {
    self.exclude_paths.insert(path);
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
          self.min_root_dir =
            Some(WindowsSystemRootablePath::root_for_current_os());
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
    let canonical = self.ensure_canonical_dir(path);
    self.add_dir_recursive_not_symlink(&canonical)
  }

  fn add_dir_recursive_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    if self.exclude_paths.contains(path) {
      return Ok(());
    }
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
      self.add_path_with_file_type(&path, file_type)?;
    }

    Ok(())
  }

  pub fn add_path(&mut self, path: &Path) -> Result<(), AnyError> {
    // ok, building fs implementation
    #[allow(clippy::disallowed_methods)]
    let file_type = path.metadata()?.file_type();
    self.add_path_with_file_type(path, file_type)
  }

  fn add_path_with_file_type(
    &mut self,
    path: &Path,
    file_type: std::fs::FileType,
  ) -> Result<(), AnyError> {
    if self.exclude_paths.contains(path) {
      return Ok(());
    }
    if file_type.is_dir() {
      self.add_dir_recursive_not_symlink(path)
    } else if file_type.is_file() {
      self.add_file_at_path_not_symlink(path)
    } else if file_type.is_symlink() {
      match self.add_symlink(path) {
        Ok(target) => match target {
          SymlinkTarget::File(target) => {
            self.add_file_at_path_not_symlink(&target)
          }
          SymlinkTarget::Dir(target) => {
            self.add_dir_recursive_not_symlink(&target)
          }
        },
        Err(err) => {
          log::warn!(
            "{} Failed resolving symlink. Ignoring.\n    Path: {}\n    Message: {:#}",
            colors::yellow("Warning"),
            path.display(),
            err
          );
          Ok(())
        }
      }
    } else {
      // ignore
      Ok(())
    }
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
        _ => unreachable!("{}", path.display()),
      };
    }

    Some(current_dir)
  }

  pub fn add_file_at_path(&mut self, path: &Path) -> Result<(), AnyError> {
    if self.exclude_paths.contains(path) {
      return Ok(());
    }
    let (file_bytes, mtime) = self.read_file_bytes_and_mtime(path)?;
    self.add_file_with_data(
      path,
      AddFileDataOptions {
        data: file_bytes,
        mtime,
        maybe_cjs_export_analysis: None,
        maybe_transpiled: None,
        maybe_source_map: None,
      },
    )
  }

  fn add_file_at_path_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    if self.exclude_paths.contains(path) {
      return Ok(());
    }
    let (file_bytes, mtime) = self.read_file_bytes_and_mtime(path)?;
    self.add_file_with_data_raw(path, file_bytes, mtime)
  }

  fn read_file_bytes_and_mtime(
    &self,
    path: &Path,
  ) -> Result<(Vec<u8>, Option<SystemTime>), AnyError> {
    // ok, building fs implementation
    #[allow(clippy::disallowed_methods)]
    {
      let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("Opening {}", path.display()))?;
      let mtime = file.metadata().ok().and_then(|m| m.modified().ok());
      let mut file_bytes = Vec::new();
      file
        .read_to_end(&mut file_bytes)
        .with_context(|| format!("Reading {}", path.display()))?;
      Ok((file_bytes, mtime))
    }
  }

  /// Canonicalizes the parent directory of the given path and, if it
  /// differs from the original, adds VFS symlink entries for any aliased
  /// directory components (e.g. Windows 8.3 short names like
  /// RUNNER~1 -> runneradmin). Only the parent is canonicalized so that
  /// symlink files themselves are not followed.
  /// Returns the path with canonical parent, or the original path if
  /// canonicalization fails or produces the same result.
  fn ensure_canonical_path<'a>(&mut self, path: &'a Path) -> Cow<'a, Path> {
    if let Some(cached) = self.canonical_path_cache.get(path) {
      return match cached {
        Some(canonical) => Cow::Owned(canonical.clone()),
        None => Cow::Borrowed(path),
      };
    }

    let Some(parent) = path.parent() else {
      return Cow::Borrowed(path);
    };
    let Some(file_name) = path.file_name() else {
      return Cow::Borrowed(path);
    };

    let canonical_parent = self.ensure_canonical_dir(parent);
    if canonical_parent == parent {
      self.canonical_path_cache.insert(path.to_path_buf(), None);
      return Cow::Borrowed(path);
    }
    let result = canonical_parent.join(file_name);
    self
      .canonical_path_cache
      .insert(path.to_path_buf(), Some(result.clone()));
    Cow::Owned(result)
  }

  /// Resolves a directory path by walking component by component, detecting
  /// symlinks (via `read_link`) and name aliases (e.g. Windows 8.3 short
  /// names). For each symlink or alias found, a VFS symlink entry is
  /// created so the VFS mirrors the real filesystem structure.
  /// Caches results (including negative) to avoid repeated filesystem calls.
  fn ensure_canonical_dir<'a>(&mut self, dir_path: &'a Path) -> Cow<'a, Path> {
    if let Some(cached) = self.canonical_path_cache.get(dir_path) {
      return match cached {
        Some(canonical) => Cow::Owned(canonical.clone()),
        None => Cow::Borrowed(dir_path),
      };
    }

    // insert negative cache upfront to prevent infinite recursion
    // if circular symlinks are encountered
    self
      .canonical_path_cache
      .insert(dir_path.to_path_buf(), None);

    let mut current_real = PathBuf::new();
    let mut changed = false;

    for component in dir_path.components() {
      if matches!(
        component,
        std::path::Component::RootDir | std::path::Component::Prefix(_)
      ) {
        current_real.push(component);
        continue;
      }

      let candidate = current_real.join(component.as_os_str());

      // ok, fs implementation
      #[allow(clippy::disallowed_methods)]
      let is_symlink = std::fs::symlink_metadata(&candidate)
        .map(|m| m.is_symlink())
        .unwrap_or(false);

      if is_symlink {
        // ok, fs implementation
        #[allow(clippy::disallowed_methods)]
        if let Ok(link_target) = std::fs::read_link(&candidate) {
          let resolved = normalize_path(Cow::Owned(
            current_real.join(strip_unc_prefix(link_target)),
          ));

          // recursively ensure the symlink target is canonical
          let resolved = self.ensure_canonical_dir(&resolved).into_owned();

          // determine if the final target is a directory
          // ok, fs implementation
          #[allow(clippy::disallowed_methods)]
          let dest_is_dir = std::fs::metadata(&candidate)
            .map(|m| m.is_dir())
            .unwrap_or(true);

          let name = component.as_os_str().to_string_lossy();
          let case_sensitivity = self.case_sensitivity;
          let dir = self.add_dir_raw(&current_real);
          dir.entries.insert_or_modify(
            &name,
            case_sensitivity,
            || {
              VfsEntry::Symlink(VirtualSymlink {
                name: name.to_string(),
                dest_parts: VirtualSymlinkParts::from_path(&resolved),
                dest_is_dir,
              })
            },
            |_| {
              // already exists
            },
          );

          log::debug!(
            "Resolved symlink component: {} -> {}",
            candidate.display(),
            resolved.display()
          );

          current_real = resolved;
          changed = true;
          continue;
        }
      }

      // not a symlink — on Windows, check for 8.3 short name aliases
      // by canonicalizing just this path and comparing the last component.
      // This is Windows-specific; on other platforms non-symlink components
      // won't have name aliases, and running this could create spurious VFS
      // symlinks for case differences on case-insensitive filesystems.
      #[cfg(windows)]
      {
        // ok, fs implementation
        #[allow(clippy::disallowed_methods)]
        if let Ok(canonical) = std::fs::canonicalize(&candidate) {
          let canonical = strip_unc_prefix(canonical);
          if let Some(canon_name) = canonical.file_name() {
            let orig_name = component.as_os_str();
            if !self.case_sensitivity.eq_name(
              &orig_name.to_string_lossy(),
              &canon_name.to_string_lossy(),
            ) {
              let real_dir = current_real.join(canon_name);
              let orig_name_str = orig_name.to_string_lossy();
              let case_sensitivity = self.case_sensitivity;
              let dir = self.add_dir_raw(&current_real);
              dir.entries.insert_or_modify(
                &orig_name_str,
                case_sensitivity,
                || {
                  VfsEntry::Symlink(VirtualSymlink {
                    name: orig_name_str.to_string(),
                    dest_parts: VirtualSymlinkParts::from_path(&real_dir),
                    dest_is_dir: true,
                  })
                },
                |_| {
                  // already exists
                },
              );

              log::debug!(
                "Resolved 8.3 name alias: {} -> {}",
                candidate.display(),
                real_dir.display()
              );

              current_real = real_dir;
              changed = true;
              continue;
            }
          }
        }
      }

      current_real.push(component);
    }

    if changed {
      self
        .canonical_path_cache
        .insert(dir_path.to_path_buf(), Some(current_real.clone()));
      Cow::Owned(current_real)
    } else {
      // already inserted None upfront
      Cow::Borrowed(dir_path)
    }
  }

  pub fn add_file_with_data(
    &mut self,
    path: &Path,
    options: AddFileDataOptions,
  ) -> Result<(), AnyError> {
    // canonicalize parent to resolve aliased directory components
    // (e.g. Windows 8.3 short names), adding VFS symlinks as needed
    let path = self.ensure_canonical_path(path);
    // ok, fs implementation
    #[allow(clippy::disallowed_methods)]
    let metadata = std::fs::symlink_metadata(&path).with_context(|| {
      format!("Resolving target path for '{}'", path.display())
    })?;
    if metadata.is_symlink() {
      let target = self.add_symlink(&path)?.into_path_buf();
      self.add_file_with_data_raw_options(&target, options)
    } else {
      self.add_file_with_data_raw_options(&path, options)
    }
  }

  pub fn add_file_with_data_raw(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    mtime: Option<SystemTime>,
  ) -> Result<(), AnyError> {
    self.add_file_with_data_raw_options(
      path,
      AddFileDataOptions {
        data,
        mtime,
        maybe_transpiled: None,
        maybe_cjs_export_analysis: None,
        maybe_source_map: None,
      },
    )
  }

  fn add_file_with_data_raw_options(
    &mut self,
    path: &Path,
    options: AddFileDataOptions,
  ) -> Result<(), AnyError> {
    log::debug!("Adding file '{}'", path.display());
    let case_sensitivity = self.case_sensitivity;

    let is_valid_utf8 = is_valid_utf8(&options.data);
    let offset_and_len = self.files.add_data(options.data);
    let transpiled_offset = options
      .maybe_transpiled
      .map(|data| self.files.add_data(data));
    let source_map_offset = options
      .maybe_source_map
      .map(|data| self.files.add_data(data));
    let cjs_export_analysis_offset = options
      .maybe_cjs_export_analysis
      .map(|data| self.files.add_data(data));
    let dir = self.add_dir_raw(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();

    let mtime = options
      .mtime
      .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
      .map(|m| m.as_millis());

    let executable = {
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        // ok, fs implementation
        #[allow(clippy::disallowed_methods)]
        std::fs::metadata(path)
          .map(|m| m.permissions().mode() & 0o111 != 0)
          .unwrap_or(false)
      }
      #[cfg(not(unix))]
      {
        false
      }
    };

    dir.entries.insert_or_modify(
      &name,
      case_sensitivity,
      || {
        VfsEntry::File(VirtualFile {
          name: name.to_string(),
          is_valid_utf8,
          offset: offset_and_len,
          transpiled_offset,
          cjs_export_analysis_offset,
          source_map_offset,
          mtime,
          executable,
        })
      },
      |entry| match entry {
        VfsEntry::File(virtual_file) => {
          virtual_file.offset = offset_and_len;
          // doesn't overwrite to None
          if transpiled_offset.is_some() {
            virtual_file.transpiled_offset = transpiled_offset;
          }
          if source_map_offset.is_some() {
            virtual_file.source_map_offset = source_map_offset;
          }
          if cjs_export_analysis_offset.is_some() {
            virtual_file.cjs_export_analysis_offset =
              cjs_export_analysis_offset;
          }
          virtual_file.mtime = mtime;
        }
        VfsEntry::Dir(_) | VfsEntry::Symlink(_) => unreachable!(),
      },
    );

    Ok(())
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
    let target =
      normalize_path(Cow::Owned(path.parent().unwrap().join(&target)));
    // use metadata (follows symlinks) to determine final target type
    #[allow(clippy::disallowed_methods)]
    let dest_is_dir = std::fs::metadata(&*target)
      .map(|m| m.is_dir())
      .unwrap_or(false);
    let dir = self.add_dir_raw(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    dir.entries.insert_or_modify(
      &name,
      case_sensitivity,
      || {
        VfsEntry::Symlink(VirtualSymlink {
          name: name.to_string(),
          dest_parts: VirtualSymlinkParts::from_path(&target),
          dest_is_dir,
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
      if !visited.insert(target.to_path_buf()) {
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
      Ok(SymlinkTarget::Dir(target.into_owned()))
    } else {
      Ok(SymlinkTarget::File(target.into_owned()))
    }
  }

  /// Adds the CJS export analysis to the provided file.
  ///
  /// Warning: This will panic if the file wasn't properly
  /// setup before calling this.
  pub fn add_cjs_export_analysis(&mut self, path: &Path, data: Vec<u8>) {
    self.add_data_for_file_or_panic(path, data, |file, offset_with_length| {
      file.cjs_export_analysis_offset = Some(offset_with_length);
    })
  }

  fn add_data_for_file_or_panic(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    update_file: impl FnOnce(&mut VirtualFile, OffsetWithLength),
  ) {
    // resolve to canonical path so we find the file even when
    // the caller uses a non-canonical path
    let path = self.ensure_canonical_path(path);
    let offset_with_length = self.files.add_data(data);
    let case_sensitivity = self.case_sensitivity;
    let dir = self.get_dir_mut(path.parent().unwrap()).unwrap();
    let name = path.file_name().unwrap().to_string_lossy();
    let file = dir
      .entries
      .get_mut_by_name(&name, case_sensitivity)
      .unwrap();
    match file {
      VfsEntry::File(virtual_file) => {
        update_file(virtual_file, offset_with_length);
      }
      VfsEntry::Dir(_) | VfsEntry::Symlink(_) => {
        unreachable!()
      }
    }
  }

  /// Iterates through all the files in the virtual file system.
  pub fn iter_files(
    &self,
  ) -> impl Iterator<Item = (PathBuf, &VirtualFile)> + '_ {
    FileIterator {
      pending_dirs: VecDeque::from([(
        WindowsSystemRootablePath::root_for_current_os(),
        &self.executable_root,
      )]),
      current_dir_index: 0,
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
    let mut current_path = WindowsSystemRootablePath::root_for_current_os();
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
      files: self.files.files,
    }
  }
}

struct FileIterator<'a> {
  pending_dirs: VecDeque<(WindowsSystemRootablePath, &'a VirtualDirectory)>,
  current_dir_index: usize,
}

impl<'a> Iterator for FileIterator<'a> {
  type Item = (PathBuf, &'a VirtualFile);

  fn next(&mut self) -> Option<Self::Item> {
    while !self.pending_dirs.is_empty() {
      let (dir_path, current_dir) = self.pending_dirs.front()?;
      if let Some(entry) =
        current_dir.entries.get_by_index(self.current_dir_index)
      {
        self.current_dir_index += 1;
        match entry {
          VfsEntry::Dir(virtual_directory) => {
            self.pending_dirs.push_back((
              WindowsSystemRootablePath::Path(
                dir_path.join(&virtual_directory.name),
              ),
              virtual_directory,
            ));
          }
          VfsEntry::File(virtual_file) => {
            return Some((dir_path.join(&virtual_file.name), virtual_file));
          }
          VfsEntry::Symlink(_) => {
            // ignore
          }
        }
      } else {
        self.pending_dirs.pop_front();
        self.current_dir_index = 0;
      }
    }
    None
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
