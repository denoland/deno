// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;

use deno_lib::standalone::virtual_fs::BuiltVfs;
use deno_lib::standalone::virtual_fs::OffsetWithLength;
use deno_lib::standalone::virtual_fs::VfsEntry;
use deno_lib::standalone::virtual_fs::VirtualDirectory;
use deno_lib::standalone::virtual_fs::VirtualDirectoryEntries;
use deno_lib::standalone::virtual_fs::VirtualFile;
use deno_lib::standalone::virtual_fs::VirtualSymlinkParts;
use deno_lib::standalone::virtual_fs::WindowsSystemRootablePath;
use deno_lib::standalone::virtual_fs::DENO_COMPILE_GLOBAL_NODE_MODULES_DIR_NAME;

use crate::util::display::human_size;
use crate::util::display::DisplayTreeNode;

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
  log::info!("{}", text.trim());
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
    Symlink(&'a VirtualSymlinkParts),
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
            format!("{} --> {}", name, parts.display())
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
    let maybe_offsets = [
      file.transpiled_offset,
      file.source_map_offset,
      file.cjs_export_analysis_offset,
    ];
    for offset in maybe_offsets.into_iter().flatten() {
      add_offset_to_size(offset, &mut size, seen_offsets);
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
              EntryOutput::Symlink(&virtual_symlink.dest_parts)
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
        EntryOutput::Symlink(&virtual_symlink.dest_parts)
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

#[cfg(test)]
mod test {
  use console_static_text::ansi::strip_ansi_codes;
  use deno_lib::standalone::virtual_fs::VfsBuilder;
  use test_util::TempDir;

  use super::*;

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
