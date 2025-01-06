// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::ResourceHandleFd;
use deno_path_util::normalize_path;
use deno_path_util::strip_unc_prefix;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_io;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;
use indexmap::IndexSet;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use super::binary::DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME;
use crate::util;
use crate::util::display::human_size;
use crate::util::display::DisplayTreeNode;
use crate::util::fs::canonicalize_path;

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

#[derive(Debug)]
pub struct BuiltVfs {
  pub root_path: WindowsSystemRootablePath,
  pub entries: VirtualDirectoryEntries,
  pub files: Vec<Vec<u8>>,
}

#[derive(Debug, Copy, Clone)]
pub enum VfsFileSubDataKind {
  /// Raw bytes of the file.
  Raw,
  /// Bytes to use for module loading. For example, for TypeScript
  /// files this will be the transpiled JavaScript source.
  ModuleGraph,
}

#[derive(Debug)]
pub struct VfsBuilder {
  executable_root: VirtualDirectory,
  files: Vec<Vec<u8>>,
  current_offset: u64,
  file_offsets: HashMap<String, u64>,
  /// The minimum root directory that should be included in the VFS.
  min_root_dir: Option<WindowsSystemRootablePath>,
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
    }
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

  fn add_dir_raw(&mut self, path: &Path) -> &mut VirtualDirectory {
    log::debug!("Ensuring directory '{}'", path.display());
    debug_assert!(path.is_absolute());
    let mut current_dir = &mut self.executable_root;

    for component in path.components() {
      if matches!(component, std::path::Component::RootDir) {
        continue;
      }
      let name = component.as_os_str().to_string_lossy();
      let index = match current_dir.entries.binary_search(&name) {
        Ok(index) => index,
        Err(insert_index) => {
          current_dir.entries.0.insert(
            insert_index,
            VfsEntry::Dir(VirtualDirectory {
              name: name.to_string(),
              entries: Default::default(),
            }),
          );
          insert_index
        }
      };
      match &mut current_dir.entries.0[index] {
        VfsEntry::Dir(dir) => {
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
      let entry = current_dir.entries.get_mut_by_name(&name)?;
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
    let file_bytes = std::fs::read(path)
      .with_context(|| format!("Reading {}", path.display()))?;
    self.add_file_with_data(path, file_bytes, VfsFileSubDataKind::Raw)
  }

  fn add_file_at_path_not_symlink(
    &mut self,
    path: &Path,
  ) -> Result<(), AnyError> {
    let file_bytes = std::fs::read(path)
      .with_context(|| format!("Reading {}", path.display()))?;
    self.add_file_with_data_inner(path, file_bytes, VfsFileSubDataKind::Raw)
  }

  pub fn add_file_with_data(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    sub_data_kind: VfsFileSubDataKind,
  ) -> Result<(), AnyError> {
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
      format!("Resolving target path for '{}'", path.display())
    })?;
    if metadata.is_symlink() {
      let target = self.add_symlink(path)?.into_path_buf();
      self.add_file_with_data_inner(&target, data, sub_data_kind)
    } else {
      self.add_file_with_data_inner(path, data, sub_data_kind)
    }
  }

  fn add_file_with_data_inner(
    &mut self,
    path: &Path,
    data: Vec<u8>,
    sub_data_kind: VfsFileSubDataKind,
  ) -> Result<(), AnyError> {
    log::debug!("Adding file '{}'", path.display());
    let checksum = util::checksum::gen(&[&data]);
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
    match dir.entries.binary_search(&name) {
      Ok(index) => {
        let entry = &mut dir.entries.0[index];
        match entry {
          VfsEntry::File(virtual_file) => match sub_data_kind {
            VfsFileSubDataKind::Raw => {
              virtual_file.offset = offset_and_len;
            }
            VfsFileSubDataKind::ModuleGraph => {
              virtual_file.module_graph_offset = offset_and_len;
            }
          },
          VfsEntry::Dir(_) | VfsEntry::Symlink(_) => unreachable!(),
        }
      }
      Err(insert_index) => {
        dir.entries.0.insert(
          insert_index,
          VfsEntry::File(VirtualFile {
            name: name.to_string(),
            offset: offset_and_len,
            module_graph_offset: offset_and_len,
          }),
        );
      }
    }

    // new file, update the list of files
    if self.current_offset == offset {
      self.files.push(data);
      self.current_offset += offset_and_len.len;
    }

    Ok(())
  }

  fn resolve_target_path(&mut self, path: &Path) -> Result<PathBuf, AnyError> {
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
      format!("Resolving target path for '{}'", path.display())
    })?;
    if metadata.is_symlink() {
      Ok(self.add_symlink(path)?.into_path_buf())
    } else {
      Ok(path.to_path_buf())
    }
  }

  fn add_symlink(&mut self, path: &Path) -> Result<SymlinkTarget, AnyError> {
    self.add_symlink_inner(path, &mut IndexSet::new())
  }

  fn add_symlink_inner(
    &mut self,
    path: &Path,
    visited: &mut IndexSet<PathBuf>,
  ) -> Result<SymlinkTarget, AnyError> {
    log::debug!("Adding symlink '{}'", path.display());
    let target = strip_unc_prefix(
      std::fs::read_link(path)
        .with_context(|| format!("Reading symlink '{}'", path.display()))?,
    );
    let target = normalize_path(path.parent().unwrap().join(&target));
    let dir = self.add_dir_raw(path.parent().unwrap());
    let name = path.file_name().unwrap().to_string_lossy();
    match dir.entries.binary_search(&name) {
      Ok(_) => {} // previously inserted
      Err(insert_index) => {
        dir.entries.0.insert(
          insert_index,
          VfsEntry::Symlink(VirtualSymlink {
            name: name.to_string(),
            dest_parts: VirtualSymlinkParts::from_path(&target),
          }),
        );
      }
    }
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
      for entry in &mut dir.entries.0 {
        match entry {
          VfsEntry::Dir(dir) => {
            strip_prefix_from_symlinks(dir, parts);
          }
          VfsEntry::File(_) => {}
          VfsEntry::Symlink(symlink) => {
            let old_parts = std::mem::take(&mut symlink.dest_parts.0);
            symlink.dest_parts.0 =
              old_parts.into_iter().skip(parts.len()).collect();
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
      match &current_dir.entries.0[0] {
        VfsEntry::Dir(dir) => {
          if dir.name == DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME {
            // special directory we want to maintain
            break;
          }
          match current_dir.entries.0.remove(0) {
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
        &VirtualSymlinkParts::from_path(path).0,
      );
    }
    BuiltVfs {
      root_path: current_path,
      entries: current_dir.entries,
      files: self.files,
    }
  }
}

#[derive(Debug)]
enum SymlinkTarget {
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

pub fn output_vfs(vfs: &BuiltVfs, executable_name: &str) {
  if !log::log_enabled!(log::Level::Info) {
    return; // no need to compute if won't output
  }

  if vfs.entries.is_empty() {
    return; // nothing to output
  }

  let mut text = String::new();
  let display_tree = vfs_as_display_tree(vfs, executable_name);
  display_tree.print(&mut text).unwrap(); // unwrap ok because it's writing to a string
  log::info!("\n{}\n", deno_terminal::colors::bold("Embedded Files"));
  log::info!("{}\n", text.trim());
  log::info!(
    "Size: {}\n",
    human_size(vfs.files.iter().map(|f| f.len() as f64).sum())
  );
}

fn vfs_as_display_tree(
  vfs: &BuiltVfs,
  executable_name: &str,
) -> DisplayTreeNode {
  /// The VFS only stores duplicate files once, so track that and display
  /// it to the user so that it's not confusing.
  #[derive(Debug, Default, Copy, Clone)]
  struct Size {
    unique: u64,
    total: u64,
  }

  impl std::ops::Add for Size {
    type Output = Self;

    fn add(self, other: Self) -> Self {
      Self {
        unique: self.unique + other.unique,
        total: self.total + other.total,
      }
    }
  }

  impl std::iter::Sum for Size {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
      iter.fold(Self::default(), std::ops::Add::add)
    }
  }

  enum EntryOutput<'a> {
    All(Size),
    Subset(Vec<DirEntryOutput<'a>>),
    File(Size),
    Symlink(&'a [String]),
  }

  impl<'a> EntryOutput<'a> {
    pub fn size(&self) -> Size {
      match self {
        EntryOutput::All(size) => *size,
        EntryOutput::Subset(children) => {
          children.iter().map(|c| c.output.size()).sum()
        }
        EntryOutput::File(size) => *size,
        EntryOutput::Symlink(_) => Size {
          unique: 0,
          total: 0,
        },
      }
    }
  }

  impl<'a> EntryOutput<'a> {
    pub fn as_display_tree(&self, name: String) -> DisplayTreeNode {
      fn format_size(size: Size) -> String {
        if size.unique == size.total {
          human_size(size.unique as f64)
        } else {
          format!(
            "{}{}",
            human_size(size.total as f64),
            deno_terminal::colors::gray(format!(
              " - {} unique",
              human_size(size.unique as f64)
            ))
          )
        }
      }

      DisplayTreeNode {
        text: match self {
          EntryOutput::All(size) => {
            format!("{}/* ({})", name, format_size(*size))
          }
          EntryOutput::Subset(children) => {
            let size = children.iter().map(|c| c.output.size()).sum::<Size>();
            format!("{} ({})", name, format_size(size))
          }
          EntryOutput::File(size) => {
            format!("{} ({})", name, format_size(*size))
          }
          EntryOutput::Symlink(parts) => {
            format!("{} --> {}", name, parts.join("/"))
          }
        },
        children: match self {
          EntryOutput::All(_) => Vec::new(),
          EntryOutput::Subset(children) => children
            .iter()
            .map(|entry| entry.output.as_display_tree(entry.name.to_string()))
            .collect(),
          EntryOutput::File(_) => Vec::new(),
          EntryOutput::Symlink(_) => Vec::new(),
        },
      }
    }
  }

  pub struct DirEntryOutput<'a> {
    name: Cow<'a, str>,
    output: EntryOutput<'a>,
  }

  impl<'a> DirEntryOutput<'a> {
    /// Collapses leaf nodes so they don't take up so much space when being
    /// displayed.
    ///
    /// We only want to collapse leafs so that nodes of the same depth have
    /// the same indentation.
    pub fn collapse_leaf_nodes(&mut self) {
      let EntryOutput::Subset(vec) = &mut self.output else {
        return;
      };
      for dir_entry in vec.iter_mut() {
        dir_entry.collapse_leaf_nodes();
      }
      if vec.len() != 1 {
        return;
      }
      let child = &mut vec[0];
      let child_name = &child.name;
      match &mut child.output {
        EntryOutput::All(size) => {
          self.name = Cow::Owned(format!("{}/{}", self.name, child_name));
          self.output = EntryOutput::All(*size);
        }
        EntryOutput::Subset(children) => {
          if children.is_empty() {
            self.name = Cow::Owned(format!("{}/{}", self.name, child_name));
            self.output = EntryOutput::Subset(vec![]);
          }
        }
        EntryOutput::File(size) => {
          self.name = Cow::Owned(format!("{}/{}", self.name, child_name));
          self.output = EntryOutput::File(*size);
        }
        EntryOutput::Symlink(parts) => {
          let new_name = format!("{}/{}", self.name, child_name);
          self.output = EntryOutput::Symlink(parts);
          self.name = Cow::Owned(new_name);
        }
      }
    }
  }

  fn file_size(file: &VirtualFile, seen_offsets: &mut HashSet<u64>) -> Size {
    fn add_offset_to_size(
      offset: OffsetWithLength,
      size: &mut Size,
      seen_offsets: &mut HashSet<u64>,
    ) {
      if offset.len == 0 {
        // some empty files have a dummy offset, so don't
        // insert them into the seen offsets
        return;
      }

      if seen_offsets.insert(offset.offset) {
        size.total += offset.len;
        size.unique += offset.len;
      } else {
        size.total += offset.len;
      }
    }

    let mut size = Size::default();
    add_offset_to_size(file.offset, &mut size, seen_offsets);
    if file.module_graph_offset.offset != file.offset.offset {
      add_offset_to_size(file.module_graph_offset, &mut size, seen_offsets);
    }
    size
  }

  fn dir_size(dir: &VirtualDirectory, seen_offsets: &mut HashSet<u64>) -> Size {
    let mut size = Size::default();
    for entry in dir.entries.iter() {
      match entry {
        VfsEntry::Dir(virtual_directory) => {
          size = size + dir_size(virtual_directory, seen_offsets);
        }
        VfsEntry::File(file) => {
          size = size + file_size(file, seen_offsets);
        }
        VfsEntry::Symlink(_) => {
          // ignore
        }
      }
    }
    size
  }

  fn show_global_node_modules_dir<'a>(
    vfs_dir: &'a VirtualDirectory,
    seen_offsets: &mut HashSet<u64>,
  ) -> Vec<DirEntryOutput<'a>> {
    fn show_subset_deep<'a>(
      vfs_dir: &'a VirtualDirectory,
      depth: usize,
      seen_offsets: &mut HashSet<u64>,
    ) -> EntryOutput<'a> {
      if depth == 0 {
        EntryOutput::All(dir_size(vfs_dir, seen_offsets))
      } else {
        EntryOutput::Subset(show_subset(vfs_dir, depth, seen_offsets))
      }
    }

    fn show_subset<'a>(
      vfs_dir: &'a VirtualDirectory,
      depth: usize,
      seen_offsets: &mut HashSet<u64>,
    ) -> Vec<DirEntryOutput<'a>> {
      vfs_dir
        .entries
        .iter()
        .map(|entry| DirEntryOutput {
          name: Cow::Borrowed(entry.name()),
          output: match entry {
            VfsEntry::Dir(virtual_directory) => {
              show_subset_deep(virtual_directory, depth - 1, seen_offsets)
            }
            VfsEntry::File(file) => {
              EntryOutput::File(file_size(file, seen_offsets))
            }
            VfsEntry::Symlink(virtual_symlink) => {
              EntryOutput::Symlink(&virtual_symlink.dest_parts.0)
            }
          },
        })
        .collect()
    }

    // in this scenario, we want to show
    // .deno_compile_node_modules/localhost/<package_name>/<version>/*
    show_subset(vfs_dir, 3, seen_offsets)
  }

  fn include_all_entries<'a>(
    dir_path: &WindowsSystemRootablePath,
    entries: &'a VirtualDirectoryEntries,
    seen_offsets: &mut HashSet<u64>,
  ) -> Vec<DirEntryOutput<'a>> {
    entries
      .iter()
      .map(|entry| DirEntryOutput {
        name: Cow::Borrowed(entry.name()),
        output: analyze_entry(dir_path.join(entry.name()), entry, seen_offsets),
      })
      .collect()
  }

  fn analyze_entry<'a>(
    path: PathBuf,
    entry: &'a VfsEntry,
    seen_offsets: &mut HashSet<u64>,
  ) -> EntryOutput<'a> {
    match entry {
      VfsEntry::Dir(virtual_directory) => {
        analyze_dir(path, virtual_directory, seen_offsets)
      }
      VfsEntry::File(file) => EntryOutput::File(file_size(file, seen_offsets)),
      VfsEntry::Symlink(virtual_symlink) => {
        EntryOutput::Symlink(&virtual_symlink.dest_parts.0)
      }
    }
  }

  fn analyze_dir<'a>(
    dir: PathBuf,
    vfs_dir: &'a VirtualDirectory,
    seen_offsets: &mut HashSet<u64>,
  ) -> EntryOutput<'a> {
    if vfs_dir.name == DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME {
      return EntryOutput::Subset(show_global_node_modules_dir(
        vfs_dir,
        seen_offsets,
      ));
    }

    let real_entry_count = std::fs::read_dir(&dir)
      .ok()
      .map(|entries| entries.flat_map(|e| e.ok()).count())
      .unwrap_or(0);
    if real_entry_count == vfs_dir.entries.len() {
      let children = vfs_dir
        .entries
        .iter()
        .map(|entry| DirEntryOutput {
          name: Cow::Borrowed(entry.name()),
          output: analyze_entry(dir.join(entry.name()), entry, seen_offsets),
        })
        .collect::<Vec<_>>();
      if children
        .iter()
        .all(|c| !matches!(c.output, EntryOutput::Subset { .. }))
      {
        EntryOutput::All(children.iter().map(|c| c.output.size()).sum())
      } else {
        EntryOutput::Subset(children)
      }
    } else if vfs_dir.name == DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME {
      EntryOutput::Subset(show_global_node_modules_dir(vfs_dir, seen_offsets))
    } else {
      EntryOutput::Subset(include_all_entries(
        &WindowsSystemRootablePath::Path(dir),
        &vfs_dir.entries,
        seen_offsets,
      ))
    }
  }

  // always include all the entries for the root directory, otherwise the
  // user might not have context about what's being shown
  let mut seen_offsets = HashSet::with_capacity(vfs.files.len());
  let mut child_entries =
    include_all_entries(&vfs.root_path, &vfs.entries, &mut seen_offsets);
  for child_entry in &mut child_entries {
    child_entry.collapse_leaf_nodes();
  }
  DisplayTreeNode {
    text: deno_terminal::colors::italic(executable_name).to_string(),
    children: child_entries
      .iter()
      .map(|entry| entry.output.as_display_tree(entry.name.to_string()))
      .collect(),
  }
}

#[derive(Debug)]
enum VfsEntryRef<'a> {
  Dir(&'a VirtualDirectory),
  File(&'a VirtualFile),
  Symlink(&'a VirtualSymlink),
}

impl VfsEntryRef<'_> {
  pub fn as_metadata(&self) -> FileBackedVfsMetadata {
    FileBackedVfsMetadata {
      file_type: match self {
        Self::Dir(_) => sys_traits::FileType::Dir,
        Self::File(_) => sys_traits::FileType::File,
        Self::Symlink(_) => sys_traits::FileType::Symlink,
      },
      name: self.name().to_string(),
      len: match self {
        Self::Dir(_) => 0,
        Self::File(file) => file.offset.len,
        Self::Symlink(_) => 0,
      },
    }
  }

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

  fn as_ref(&self) -> VfsEntryRef {
    match self {
      VfsEntry::Dir(dir) => VfsEntryRef::Dir(dir),
      VfsEntry::File(file) => VfsEntryRef::File(file),
      VfsEntry::Symlink(symlink) => VfsEntryRef::Symlink(symlink),
    }
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

  pub fn take_inner(&mut self) -> Vec<VfsEntry> {
    std::mem::take(&mut self.0)
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn get_by_name(&self, name: &str) -> Option<&VfsEntry> {
    self.binary_search(name).ok().map(|index| &self.0[index])
  }

  pub fn get_mut_by_name(&mut self, name: &str) -> Option<&mut VfsEntry> {
    self
      .binary_search(name)
      .ok()
      .map(|index| &mut self.0[index])
  }

  pub fn binary_search(&self, name: &str) -> Result<usize, usize> {
    self.0.binary_search_by(|e| e.name().cmp(name))
  }

  pub fn insert(&mut self, entry: VfsEntry) {
    match self.binary_search(entry.name()) {
      Ok(index) => {
        self.0[index] = entry;
      }
      Err(insert_index) => {
        self.0.insert(insert_index, entry);
      }
    }
  }

  pub fn remove(&mut self, index: usize) -> VfsEntry {
    self.0.remove(index)
  }

  pub fn iter(&self) -> std::slice::Iter<'_, VfsEntry> {
    self.0.iter()
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
      let component = component.as_os_str();
      let current_dir = match current_entry {
        VfsEntryRef::Dir(dir) => {
          final_path.push(component);
          dir
        }
        VfsEntryRef::Symlink(symlink) => {
          let dest = symlink.resolve_dest_from_root(&self.root_path);
          let (resolved_path, entry) = self.find_entry_inner(&dest, seen)?;
          final_path = resolved_path; // overwrite with the new resolved path
          match entry {
            VfsEntryRef::Dir(dir) => {
              final_path.push(component);
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
      let component = component.to_string_lossy();
      current_entry = current_dir
        .entries
        .get_by_name(&component)
        .ok_or_else(|| {
          std::io::Error::new(std::io::ErrorKind::NotFound, "path not found")
        })?
        .as_ref();
    }

    Ok((final_path, current_entry))
  }
}

pub struct FileBackedVfsFile {
  file: VirtualFile,
  pos: RefCell<u64>,
  vfs: Arc<FileBackedVfs>,
}

impl FileBackedVfsFile {
  pub fn seek(&self, pos: SeekFrom) -> std::io::Result<u64> {
    match pos {
      SeekFrom::Start(pos) => {
        *self.pos.borrow_mut() = pos;
        Ok(pos)
      }
      SeekFrom::End(offset) => {
        if offset < 0 && -offset as u64 > self.file.offset.len {
          let msg = "An attempt was made to move the file pointer before the beginning of the file.";
          Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            msg,
          ))
        } else {
          let mut current_pos = self.pos.borrow_mut();
          *current_pos = if offset >= 0 {
            self.file.offset.len - (offset as u64)
          } else {
            self.file.offset.len + (-offset as u64)
          };
          Ok(*current_pos)
        }
      }
      SeekFrom::Current(offset) => {
        let mut current_pos = self.pos.borrow_mut();
        if offset >= 0 {
          *current_pos += offset as u64;
        } else if -offset as u64 > *current_pos {
          return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "An attempt was made to move the file pointer before the beginning of the file."));
        } else {
          *current_pos -= -offset as u64;
        }
        Ok(*current_pos)
      }
    }
  }

  pub fn read_to_buf(&self, buf: &mut [u8]) -> std::io::Result<usize> {
    let read_pos = {
      let mut pos = self.pos.borrow_mut();
      let read_pos = *pos;
      // advance the position due to the read
      *pos = std::cmp::min(self.file.offset.len, *pos + buf.len() as u64);
      read_pos
    };
    self.vfs.read_file(&self.file, read_pos, buf)
  }

  fn read_to_end(&self) -> FsResult<Cow<'static, [u8]>> {
    let read_pos = {
      let mut pos = self.pos.borrow_mut();
      let read_pos = *pos;
      // todo(dsherret): should this always set it to the end of the file?
      if *pos < self.file.offset.len {
        // advance the position due to the read
        *pos = self.file.offset.len;
      }
      read_pos
    };
    if read_pos > self.file.offset.len {
      return Ok(Cow::Borrowed(&[]));
    }
    if read_pos == 0 {
      Ok(
        self
          .vfs
          .read_file_all(&self.file, VfsFileSubDataKind::Raw)?,
      )
    } else {
      let size = (self.file.offset.len - read_pos) as usize;
      let mut buf = vec![0; size];
      self.vfs.read_file(&self.file, read_pos, &mut buf)?;
      Ok(Cow::Owned(buf))
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_io::fs::File for FileBackedVfsFile {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize> {
    self.read_to_buf(buf).map_err(Into::into)
  }
  async fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)> {
    // this is fast, no need to spawn a task
    let nread = self.read_to_buf(&mut buf)?;
    Ok((nread, buf))
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

  fn read_all_sync(self: Rc<Self>) -> FsResult<Cow<'static, [u8]>> {
    self.read_to_end()
  }
  async fn read_all_async(self: Rc<Self>) -> FsResult<Cow<'static, [u8]>> {
    // this is fast, no need to spawn a task
    self.read_to_end()
  }

  fn chmod_sync(self: Rc<Self>, _pathmode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chmod_async(self: Rc<Self>, _mode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn seek_sync(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos).map_err(|err| err.into())
  }
  async fn seek_async(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos).map_err(|err| err.into())
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

#[derive(Debug, Clone)]
pub struct FileBackedVfsDirEntry {
  pub parent_path: PathBuf,
  pub metadata: FileBackedVfsMetadata,
}

#[derive(Debug, Clone)]
pub struct FileBackedVfsMetadata {
  pub name: String,
  pub file_type: sys_traits::FileType,
  pub len: u64,
}

impl FileBackedVfsMetadata {
  pub fn as_fs_stat(&self) -> FsStat {
    FsStat {
      is_directory: self.file_type == sys_traits::FileType::Dir,
      is_file: self.file_type == sys_traits::FileType::File,
      is_symlink: self.file_type == sys_traits::FileType::Symlink,
      atime: None,
      birthtime: None,
      mtime: None,
      ctime: None,
      blksize: 0,
      size: self.len,
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
    }
  }
}

#[derive(Debug)]
pub struct FileBackedVfs {
  vfs_data: Cow<'static, [u8]>,
  fs_root: VfsRoot,
}

impl FileBackedVfs {
  pub fn new(data: Cow<'static, [u8]>, fs_root: VfsRoot) -> Self {
    Self {
      vfs_data: data,
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
  ) -> std::io::Result<FileBackedVfsFile> {
    let file = self.file_entry(path)?;
    Ok(FileBackedVfsFile {
      file: file.clone(),
      vfs: self.clone(),
      pos: Default::default(),
    })
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

  pub fn read_dir_with_metadata<'a>(
    &'a self,
    path: &Path,
  ) -> std::io::Result<impl Iterator<Item = FileBackedVfsDirEntry> + 'a> {
    let dir = self.dir_entry(path)?;
    let path = path.to_path_buf();
    Ok(dir.entries.iter().map(move |entry| FileBackedVfsDirEntry {
      parent_path: path.to_path_buf(),
      metadata: entry.as_ref().as_metadata(),
    }))
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

  pub fn lstat(&self, path: &Path) -> std::io::Result<FileBackedVfsMetadata> {
    let (_, entry) = self.fs_root.find_entry_no_follow(path)?;
    Ok(entry.as_metadata())
  }

  pub fn stat(&self, path: &Path) -> std::io::Result<FileBackedVfsMetadata> {
    let (_, entry) = self.fs_root.find_entry(path)?;
    Ok(entry.as_metadata())
  }

  pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (path, _) = self.fs_root.find_entry(path)?;
    Ok(path)
  }

  pub fn read_file_all(
    &self,
    file: &VirtualFile,
    sub_data_kind: VfsFileSubDataKind,
  ) -> std::io::Result<Cow<'static, [u8]>> {
    let read_len = match sub_data_kind {
      VfsFileSubDataKind::Raw => file.offset.len,
      VfsFileSubDataKind::ModuleGraph => file.module_graph_offset.len,
    };
    let read_range = self.get_read_range(file, sub_data_kind, 0, read_len)?;
    match &self.vfs_data {
      Cow::Borrowed(data) => Ok(Cow::Borrowed(&data[read_range])),
      Cow::Owned(data) => Ok(Cow::Owned(data[read_range].to_vec())),
    }
  }

  pub fn read_file(
    &self,
    file: &VirtualFile,
    pos: u64,
    buf: &mut [u8],
  ) -> std::io::Result<usize> {
    let read_range = self.get_read_range(
      file,
      VfsFileSubDataKind::Raw,
      pos,
      buf.len() as u64,
    )?;
    let read_len = read_range.len();
    buf[..read_len].copy_from_slice(&self.vfs_data[read_range]);
    Ok(read_len)
  }

  fn get_read_range(
    &self,
    file: &VirtualFile,
    sub_data_kind: VfsFileSubDataKind,
    pos: u64,
    len: u64,
  ) -> std::io::Result<Range<usize>> {
    let file_offset_and_len = match sub_data_kind {
      VfsFileSubDataKind::Raw => file.offset,
      VfsFileSubDataKind::ModuleGraph => file.module_graph_offset,
    };
    if pos > file_offset_and_len.len {
      return Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "unexpected EOF",
      ));
    }
    let file_offset =
      self.fs_root.start_file_offset + file_offset_and_len.offset;
    let start = file_offset + pos;
    let end = file_offset + std::cmp::min(pos + len, file_offset_and_len.len);
    Ok(start as usize..end as usize)
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

  use console_static_text::ansi::strip_ansi_codes;
  use deno_io::fs::File;
  use test_util::assert_contains;
  use test_util::TempDir;

  use super::*;

  #[track_caller]
  fn read_file(vfs: &FileBackedVfs, path: &Path) -> String {
    let file = vfs.file_entry(path).unwrap();
    String::from_utf8(
      vfs
        .read_file_all(file, VfsFileSubDataKind::Raw)
        .unwrap()
        .into_owned(),
    )
    .unwrap()
  }

  #[test]
  fn builds_and_uses_virtual_fs() {
    let temp_dir = TempDir::new();
    // we canonicalize the temp directory because the vfs builder
    // will canonicalize the root path
    let src_path = temp_dir.path().canonicalize().join("src");
    src_path.create_dir_all();
    src_path.join("sub_dir").create_dir_all();
    src_path.join("e.txt").write("e");
    src_path.symlink_file("e.txt", "sub_dir/e.txt");
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new();
    builder
      .add_file_with_data_inner(
        &src_path.join("a.txt"),
        "data".into(),
        VfsFileSubDataKind::Raw,
      )
      .unwrap();
    builder
      .add_file_with_data_inner(
        &src_path.join("b.txt"),
        "data".into(),
        VfsFileSubDataKind::Raw,
      )
      .unwrap();
    assert_eq!(builder.files.len(), 1); // because duplicate data
    builder
      .add_file_with_data_inner(
        &src_path.join("c.txt"),
        "c".into(),
        VfsFileSubDataKind::Raw,
      )
      .unwrap();
    builder
      .add_file_with_data_inner(
        &src_path.join("sub_dir").join("d.txt"),
        "d".into(),
        VfsFileSubDataKind::Raw,
      )
      .unwrap();
    builder.add_file_at_path(&src_path.join("e.txt")).unwrap();
    builder
      .add_symlink(&src_path.join("sub_dir").join("e.txt"))
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
    assert_eq!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Symlink,
    );
    assert_eq!(
      virtual_fs
        .stat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .file_type,
      sys_traits::FileType::File,
    );
    assert_eq!(
      virtual_fs
        .stat(&dest_path.join("sub_dir"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Dir,
    );
    assert_eq!(
      virtual_fs.stat(&dest_path.join("e.txt")).unwrap().file_type,
      sys_traits::FileType::File
    );
  }

  #[test]
  fn test_include_dir_recursive() {
    let temp_dir = TempDir::new();
    let temp_dir_path = temp_dir.path().canonicalize();
    temp_dir.create_dir_all("src/nested/sub_dir");
    temp_dir.write("src/a.txt", "data");
    temp_dir.write("src/b.txt", "data");
    util::fs::symlink_dir(
      &crate::sys::CliSys::default(),
      temp_dir_path.join("src/nested/sub_dir").as_path(),
      temp_dir_path.join("src/sub_dir_link").as_path(),
    )
    .unwrap();
    temp_dir.write("src/nested/sub_dir/c.txt", "c");

    // build and create the virtual fs
    let src_path = temp_dir_path.join("src").to_path_buf();
    let mut builder = VfsBuilder::new();
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
    assert_eq!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir_link"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Symlink,
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
    let vfs = builder.build();
    {
      let mut file = std::fs::File::create(&virtual_fs_file).unwrap();
      for file_data in &vfs.files {
        file.write_all(file_data).unwrap();
      }
    }
    let dest_path = temp_dir.path().join("dest");
    let data = std::fs::read(&virtual_fs_file).unwrap();
    (
      dest_path.to_path_buf(),
      FileBackedVfs::new(
        Cow::Owned(data),
        VfsRoot {
          dir: VirtualDirectory {
            name: "".to_string(),
            entries: vfs.entries,
          },
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
    src_path.symlink_file("a.txt", "b.txt");
    src_path.symlink_file("b.txt", "c.txt");
    src_path.symlink_file("c.txt", "a.txt");
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new();
    let err = builder
      .add_symlink(src_path.join("a.txt").as_path())
      .unwrap_err();
    assert_contains!(err.to_string(), "Circular symlink detected",);
  }

  #[tokio::test]
  async fn test_open_file() {
    let temp_dir = TempDir::new();
    let temp_path = temp_dir.path().canonicalize();
    let mut builder = VfsBuilder::new();
    builder
      .add_file_with_data_inner(
        temp_path.join("a.txt").as_path(),
        "0123456789".to_string().into_bytes(),
        VfsFileSubDataKind::Raw,
      )
      .unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);
    let virtual_fs = Arc::new(virtual_fs);
    let file = virtual_fs.open_file(&dest_path.join("a.txt")).unwrap();
    file.seek(SeekFrom::Current(2)).unwrap();
    let mut buf = vec![0; 2];
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"45");
    file.seek(SeekFrom::Current(-4)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.seek(SeekFrom::Start(2)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.seek(SeekFrom::End(2)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"89");
    file.seek(SeekFrom::Current(-8)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    assert_eq!(
      file
        .seek(SeekFrom::Current(-5))
        .unwrap_err()
        .to_string(),
      "An attempt was made to move the file pointer before the beginning of the file."
    );
    // go beyond the file length, then back
    file.seek(SeekFrom::Current(40)).unwrap();
    file.seek(SeekFrom::Current(-38)).unwrap();
    let file = Rc::new(file);
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

  #[test]
  fn test_vfs_as_display_tree() {
    let temp_dir = TempDir::new();
    temp_dir.write("root.txt", "");
    temp_dir.create_dir_all("a");
    temp_dir.write("a/a.txt", "data");
    temp_dir.write("a/b.txt", "other data");
    temp_dir.create_dir_all("b");
    temp_dir.write("b/a.txt", "");
    temp_dir.write("b/b.txt", "");
    temp_dir.create_dir_all("c");
    temp_dir.write("c/a.txt", "contents");
    temp_dir.symlink_file("c/a.txt", "c/b.txt");
    assert_eq!(temp_dir.read_to_string("c/b.txt"), "contents"); // ensure the symlink works
    let mut vfs_builder = VfsBuilder::new();
    // full dir
    vfs_builder
      .add_dir_recursive(temp_dir.path().join("a").as_path())
      .unwrap();
    // part of the dir
    vfs_builder
      .add_file_at_path(temp_dir.path().join("b/a.txt").as_path())
      .unwrap();
    // symlink
    vfs_builder
      .add_dir_recursive(temp_dir.path().join("c").as_path())
      .unwrap();
    temp_dir.write("c/c.txt", ""); // write an extra file so it shows the whole directory
    let node = vfs_as_display_tree(&vfs_builder.build(), "executable");
    let mut text = String::new();
    node.print(&mut text).unwrap();
    assert_eq!(
      strip_ansi_codes(&text),
      r#"executable
├── a/* (14B)
├── b/a.txt (0B)
└─┬ c (8B)
  ├── a.txt (8B)
  └── b.txt --> c/a.txt
"#
    );
  }
}
