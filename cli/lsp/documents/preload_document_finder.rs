// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::util::glob;
use deno_ast::MediaType;
use deno_core::ModuleSpecifier;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::fs::ReadDir;
use std::path::Path;
use std::path::PathBuf;

enum PendingEntry {
  /// File specified as a root url.
  SpecifiedRootFile(PathBuf),
  /// Directory that is queued to read.
  Dir(PathBuf),
  /// The current directory being read.
  ReadDir(Box<ReadDir>),
}

pub struct PreloadDocumentFinderOptions {
  pub enabled_paths: Vec<PathBuf>,
  pub disabled_paths: Vec<PathBuf>,
  pub limit: usize,
}

/// Iterator that finds documents that can be preloaded into
/// the LSP on startup.
pub struct PreloadDocumentFinder {
  limit: usize,
  entry_count: usize,
  pending_entries: VecDeque<PendingEntry>,
  disabled_globs: glob::GlobSet,
  disabled_paths: HashSet<PathBuf>,
}

impl PreloadDocumentFinder {
  pub fn new(options: PreloadDocumentFinderOptions) -> Self {
    fn paths_into_globs_and_paths(
      input_paths: Vec<PathBuf>,
    ) -> (glob::GlobSet, HashSet<PathBuf>) {
      let mut globs = Vec::with_capacity(input_paths.len());
      let mut paths = HashSet::with_capacity(input_paths.len());
      for path in input_paths {
        if let Ok(Some(glob)) =
          glob::GlobPattern::new_if_pattern(&path.to_string_lossy())
        {
          globs.push(glob);
        } else {
          paths.insert(path);
        }
      }
      (glob::GlobSet::new(globs), paths)
    }

    fn is_allowed_root_dir(dir_path: &Path) -> bool {
      if dir_path.parent().is_none() {
        // never search the root directory of a drive
        return false;
      }
      true
    }

    let (disabled_globs, disabled_paths) =
      paths_into_globs_and_paths(options.disabled_paths);
    let mut finder = PreloadDocumentFinder {
      limit: options.limit,
      entry_count: 0,
      pending_entries: Default::default(),
      disabled_globs,
      disabled_paths,
    };

    // initialize the finder with the initial paths
    let mut dirs = Vec::with_capacity(options.enabled_paths.len());
    for path in options.enabled_paths {
      if !finder.disabled_paths.contains(&path)
        && !finder.disabled_globs.matches_path(&path)
      {
        if path.is_dir() {
          if is_allowed_root_dir(&path) {
            dirs.push(path);
          }
        } else {
          finder
            .pending_entries
            .push_back(PendingEntry::SpecifiedRootFile(path));
        }
      }
    }
    for dir in sort_and_remove_non_leaf_dirs(dirs) {
      finder.pending_entries.push_back(PendingEntry::Dir(dir));
    }
    finder
  }

  pub fn hit_limit(&self) -> bool {
    self.entry_count >= self.limit
  }

  fn get_valid_specifier(path: &Path) -> Option<ModuleSpecifier> {
    fn is_allowed_media_type(media_type: MediaType) -> bool {
      match media_type {
        MediaType::JavaScript
        | MediaType::Jsx
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => true,
        MediaType::Json // ignore because json never depends on other files
        | MediaType::Wasm
        | MediaType::SourceMap
        | MediaType::TsBuildInfo
        | MediaType::Unknown => false,
      }
    }

    let media_type = MediaType::from_path(path);
    if is_allowed_media_type(media_type) {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
        return Some(specifier);
      }
    }
    None
  }
}

impl Iterator for PreloadDocumentFinder {
  type Item = ModuleSpecifier;

  fn next(&mut self) -> Option<Self::Item> {
    fn is_discoverable_dir(dir_path: &Path) -> bool {
      if let Some(dir_name) = dir_path.file_name() {
        let dir_name = dir_name.to_string_lossy().to_lowercase();
        // We ignore these directories by default because there is a
        // high likelihood they aren't relevant. Someone can opt-into
        // them by specifying one of them as an enabled path.
        if matches!(dir_name.as_str(), "node_modules" | ".git") {
          return false;
        }

        // ignore cargo target directories for anyone using Deno with Rust
        if dir_name == "target"
          && dir_path
            .parent()
            .map(|p| p.join("Cargo.toml").exists())
            .unwrap_or(false)
        {
          return false;
        }

        true
      } else {
        false
      }
    }

    fn is_discoverable_file(file_path: &Path) -> bool {
      // Don't auto-discover minified files as they are likely to be very large
      // and likely not to have dependencies on code outside them that would
      // be useful in the LSP
      if let Some(file_name) = file_path.file_name() {
        let file_name = file_name.to_string_lossy().to_lowercase();
        !file_name.as_str().contains(".min.")
      } else {
        false
      }
    }

    while let Some(entry) = self.pending_entries.pop_front() {
      match entry {
        PendingEntry::SpecifiedRootFile(file) => {
          // since it was a file that was specified as a root url, only
          // verify that it's valid
          if let Some(specifier) = Self::get_valid_specifier(&file) {
            return Some(specifier);
          }
        }
        PendingEntry::Dir(dir_path) => {
          if let Ok(read_dir) = fs::read_dir(&dir_path) {
            self
              .pending_entries
              .push_back(PendingEntry::ReadDir(Box::new(read_dir)));
          }
        }
        PendingEntry::ReadDir(mut entries) => {
          while let Some(entry) = entries.next() {
            self.entry_count += 1;

            if self.hit_limit() {
              self.pending_entries.clear(); // stop searching
              return None;
            }

            if let Ok(entry) = entry {
              let path = entry.path();
              if let Ok(file_type) = entry.file_type() {
                if !self.disabled_paths.contains(&path)
                  && !self.disabled_globs.matches_path(&path)
                {
                  if file_type.is_dir() && is_discoverable_dir(&path) {
                    self
                      .pending_entries
                      .push_back(PendingEntry::Dir(path.to_path_buf()));
                  } else if file_type.is_file() && is_discoverable_file(&path) {
                    if let Some(specifier) = Self::get_valid_specifier(&path) {
                      // restore the next entries for next time
                      self
                        .pending_entries
                        .push_front(PendingEntry::ReadDir(entries));
                      return Some(specifier);
                    }
                  }
                }
              }
            }
          }
        }
      }
    }

    None
  }
}

/// Removes any directories that are a descendant of another directory in the collection.
pub fn sort_and_remove_non_leaf_dirs(mut dirs: Vec<PathBuf>) -> Vec<PathBuf> {
  if dirs.is_empty() {
    return dirs;
  }

  dirs.sort();
  if !dirs.is_empty() {
    for i in (0..dirs.len() - 1).rev() {
      let prev = &dirs[i + 1];
      if prev.starts_with(&dirs[i]) {
        dirs.remove(i + 1);
      }
    }
  }

  dirs
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  #[test]
  pub fn test_pre_load_document_finder() {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all("root1/node_modules/");
    temp_dir.write("root1/node_modules/mod.ts", ""); // no, node_modules

    temp_dir.create_dir_all("root1/sub_dir");
    temp_dir.create_dir_all("root1/target");
    temp_dir.create_dir_all("root1/node_modules");
    temp_dir.create_dir_all("root1/.git");
    temp_dir.create_dir_all("root1/file.ts"); // no, directory
    temp_dir.write("root1/mod1.ts", ""); // yes
    temp_dir.write("root1/mod2.js", ""); // yes
    temp_dir.write("root1/mod3.tsx", ""); // yes
    temp_dir.write("root1/mod4.d.ts", ""); // yes
    temp_dir.write("root1/mod5.jsx", ""); // yes
    temp_dir.write("root1/mod6.mjs", ""); // yes
    temp_dir.write("root1/mod7.mts", ""); // yes
    temp_dir.write("root1/mod8.d.mts", ""); // yes
    temp_dir.write("root1/other.json", ""); // no, json
    temp_dir.write("root1/other.txt", ""); // no, text file
    temp_dir.write("root1/other.wasm", ""); // no, don't load wasm
    temp_dir.write("root1/Cargo.toml", ""); // no
    temp_dir.write("root1/sub_dir/mod.ts", ""); // yes
    temp_dir.write("root1/sub_dir/data.min.ts", ""); // no, minified file
    temp_dir.write("root1/.git/main.ts", ""); // no, .git folder
    temp_dir.write("root1/node_modules/main.ts", ""); // no, because it's in a node_modules folder
    temp_dir.write("root1/target/main.ts", ""); // no, because there is a Cargo.toml in the root directory

    temp_dir.create_dir_all("root2/folder");
    temp_dir.create_dir_all("root2/sub_folder");
    temp_dir.write("root2/file1.ts", ""); // yes, provided
    temp_dir.write("root2/file2.ts", ""); // no, not provided
    temp_dir.write("root2/main.min.ts", ""); // yes, provided
    temp_dir.write("root2/folder/main.ts", ""); // yes, provided
    temp_dir.write("root2/sub_folder/a.js", ""); // no, not provided
    temp_dir.write("root2/sub_folder/b.ts", ""); // no, not provided
    temp_dir.write("root2/sub_folder/c.js", ""); // no, not provided

    temp_dir.create_dir_all("root3/");
    temp_dir.write("root3/mod.ts", ""); // no, not provided

    let mut urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![
        temp_dir.path().to_path_buf().join("root1"),
        temp_dir.path().to_path_buf().join("root2").join("file1.ts"),
        temp_dir
          .path()
          .to_path_buf()
          .join("root2")
          .join("main.min.ts"),
        temp_dir.path().to_path_buf().join("root2").join("folder"),
      ],
      disabled_paths: Vec::new(),
      limit: 1_000,
    })
    .collect::<Vec<_>>();

    // Ideally we would test for order here, which should be BFS, but
    // different file systems have different directory iteration
    // so we sort the results
    urls.sort();

    assert_eq!(
      urls,
      vec![
        temp_dir.uri().join("root1/mod1.ts").unwrap(),
        temp_dir.uri().join("root1/mod2.js").unwrap(),
        temp_dir.uri().join("root1/mod3.tsx").unwrap(),
        temp_dir.uri().join("root1/mod4.d.ts").unwrap(),
        temp_dir.uri().join("root1/mod5.jsx").unwrap(),
        temp_dir.uri().join("root1/mod6.mjs").unwrap(),
        temp_dir.uri().join("root1/mod7.mts").unwrap(),
        temp_dir.uri().join("root1/mod8.d.mts").unwrap(),
        temp_dir.uri().join("root1/sub_dir/mod.ts").unwrap(),
        temp_dir.uri().join("root2/file1.ts").unwrap(),
        temp_dir.uri().join("root2/folder/main.ts").unwrap(),
        temp_dir.uri().join("root2/main.min.ts").unwrap(),
      ]
    );

    // now try iterating with a low limit
    let urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![temp_dir.path().to_path_buf()],
      disabled_paths: Vec::new(),
      limit: 10, // entries and not results
    })
    .collect::<Vec<_>>();

    // since different file system have different iteration
    // order, the number here may vary, so just assert it's below
    // a certain amount
    assert!(urls.len() < 5, "Actual length: {}", urls.len());

    // now try with certain directories and files disabled
    let mut urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![temp_dir.path().to_path_buf()],
      disabled_paths: vec![
        temp_dir.path().to_path_buf().join("root1"),
        temp_dir.path().to_path_buf().join("root2").join("file1.ts"),
        temp_dir.path().to_path_buf().join("**/*.js"), // ignore js files
      ],
      limit: 1_000,
    })
    .collect::<Vec<_>>();
    urls.sort();
    assert_eq!(
      urls,
      vec![
        temp_dir.uri().join("root2/file2.ts").unwrap(),
        temp_dir.uri().join("root2/folder/main.ts").unwrap(),
        temp_dir.uri().join("root2/sub_folder/b.ts").unwrap(), // won't have the javascript files
        temp_dir.uri().join("root3/mod.ts").unwrap(),
      ]
    );
  }

  #[test]
  pub fn test_pre_load_document_finder_disallowed_dirs() {
    if cfg!(windows) {
      let paths = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
        enabled_paths: vec![PathBuf::from("C:\\")],
        disabled_paths: Vec::new(),
        limit: 1_000,
      })
      .collect::<Vec<_>>();
      assert_eq!(paths, vec![]);
    } else {
      let paths = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
        enabled_paths: vec![PathBuf::from("/")],
        disabled_paths: Vec::new(),
        limit: 1_000,
      })
      .collect::<Vec<_>>();
      assert_eq!(paths, vec![]);
    }
  }

  #[test]
  fn test_sort_and_remove_non_leaf_dirs() {
    fn run_test(paths: Vec<&str>, expected_output: Vec<&str>) {
      let paths = sort_and_remove_non_leaf_dirs(
        paths.into_iter().map(PathBuf::from).collect(),
      );
      let dirs: Vec<_> =
        paths.iter().map(|dir| dir.to_string_lossy()).collect();
      assert_eq!(dirs, expected_output);
    }

    run_test(
      vec![
        "/test/asdf/test/asdf/",
        "/test/asdf/test/asdf/test.ts",
        "/test/asdf/",
        "/test/asdf/",
        "/testing/456/893/",
        "/testing/456/893/test/",
      ],
      vec!["/test/asdf/", "/testing/456/893/"],
    );
    run_test(vec![], vec![]);
  }
}
