// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_path_util::normalize_path;
use sys_traits::FsDirEntry;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsRead;
use sys_traits::FsReadDir;

use super::FilePatterns;
use crate::glob::FilePatternsMatch;
use crate::glob::PathKind;
use crate::glob::PathOrPattern;
use crate::glob::gitignore::DirGitIgnores;
use crate::glob::gitignore::GitIgnoreTree;

#[derive(Debug, Clone)]
pub struct WalkEntry<'a> {
  pub path: &'a Path,
  pub metadata: &'a dyn FsMetadataValue,
  pub patterns: &'a FilePatterns,
}

/// Collects file paths that satisfy the given predicate, by recursively walking `files`.
/// If the walker visits a path that is listed in `ignore`, it skips descending into the directory.
pub struct FileCollector<TFilter: Fn(WalkEntry) -> bool> {
  file_filter: TFilter,
  ignore_git_folder: bool,
  ignore_node_modules: bool,
  vendor_folder: Option<PathBuf>,
  use_gitignore: bool,
}

impl<TFilter: Fn(WalkEntry) -> bool> FileCollector<TFilter> {
  pub fn new(file_filter: TFilter) -> Self {
    Self {
      file_filter,
      ignore_git_folder: false,
      ignore_node_modules: false,
      vendor_folder: None,
      use_gitignore: false,
    }
  }

  pub fn ignore_node_modules(mut self) -> Self {
    self.ignore_node_modules = true;
    self
  }

  pub fn set_vendor_folder(mut self, vendor_folder: Option<PathBuf>) -> Self {
    self.vendor_folder = vendor_folder;
    self
  }

  pub fn ignore_git_folder(mut self) -> Self {
    self.ignore_git_folder = true;
    self
  }

  pub fn use_gitignore(mut self) -> Self {
    self.use_gitignore = true;
    self
  }

  pub fn collect_file_patterns<TSys: FsRead + FsMetadata + FsReadDir>(
    &self,
    sys: &TSys,
    file_patterns: &FilePatterns,
  ) -> Vec<PathBuf> {
    fn is_pattern_matched(
      maybe_git_ignore: Option<&DirGitIgnores>,
      path: &Path,
      is_dir: bool,
      file_patterns: &FilePatterns,
    ) -> bool {
      let path_kind = match is_dir {
        true => PathKind::Directory,
        false => PathKind::File,
      };
      match file_patterns.matches_path_detail(path, path_kind) {
        FilePatternsMatch::Passed => {
          // check gitignore
          let is_gitignored = maybe_git_ignore
            .as_ref()
            .map(|git_ignore| git_ignore.is_ignored(path, is_dir))
            .unwrap_or(false);
          !is_gitignored
        }
        FilePatternsMatch::PassedOptedOutExclude => true,
        FilePatternsMatch::Excluded => false,
      }
    }

    let mut maybe_git_ignores = if self.use_gitignore {
      // Override explicitly specified include paths in the
      // .gitignore file. This does not apply to globs because
      // that is way too complicated to reason about.
      let include_paths = file_patterns
        .include
        .as_ref()
        .map(|include| {
          include
            .inner()
            .iter()
            .filter_map(|path_or_pattern| {
              if let PathOrPattern::Path(p) = path_or_pattern {
                Some(p.clone())
              } else {
                None
              }
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      Some(GitIgnoreTree::new(sys, include_paths))
    } else {
      None
    };
    let mut target_files = Vec::new();
    let mut visited_paths: HashSet<PathBuf> = HashSet::default();
    let file_patterns_by_base = file_patterns.split_by_base();
    for file_patterns in file_patterns_by_base {
      let specified_path = normalize_path(Cow::Borrowed(&file_patterns.base));
      let mut pending_dirs = VecDeque::new();
      let mut handle_entry =
        |path: PathBuf,
         metadata: &dyn FsMetadataValue,
         pending_dirs: &mut VecDeque<PathBuf>| {
          let maybe_gitignore =
            maybe_git_ignores.as_mut().and_then(|git_ignores| {
              if metadata.file_type().is_dir() {
                git_ignores.get_resolved_git_ignore_for_dir(&path)
              } else {
                git_ignores.get_resolved_git_ignore_for_file(&path)
              }
            });
          if !is_pattern_matched(
            maybe_gitignore.as_deref(),
            &path,
            metadata.file_type().is_dir(),
            &file_patterns,
          ) {
            // ignore
          } else if metadata.file_type().is_dir() {
            // allow the user to opt out of ignoring by explicitly specifying the dir
            let opt_out_ignore = specified_path == path;
            let should_ignore_dir =
              !opt_out_ignore && self.is_ignored_dir(&path);
            if !should_ignore_dir && visited_paths.insert(path.clone()) {
              pending_dirs.push_back(path);
            }
          } else if (self.file_filter)(WalkEntry {
            path: &path,
            metadata,
            patterns: &file_patterns,
          }) && visited_paths.insert(path.clone())
          {
            target_files.push(path);
          }
        };

      if let Ok(metadata) = sys.fs_metadata(&specified_path) {
        handle_entry(
          specified_path.to_path_buf(),
          &metadata,
          &mut pending_dirs,
        );
      }

      // use an iterator in order to minimize the number of file system operations
      while let Some(next_dir) = pending_dirs.pop_front() {
        let Ok(entries) = sys.fs_read_dir(&next_dir) else {
          continue;
        };
        for entry in entries {
          let Ok(entry) = entry else {
            continue;
          };
          let Ok(metadata) = entry.metadata() else {
            continue;
          };
          handle_entry(entry.path().into_owned(), &metadata, &mut pending_dirs)
        }
      }
    }
    target_files
  }

  fn is_ignored_dir(&self, path: &Path) -> bool {
    path
      .file_name()
      .map(|dir_name| {
        let dir_name = dir_name.to_string_lossy().to_lowercase();

        match dir_name.as_str() {
          "node_modules" => self.ignore_node_modules,
          ".git" => self.ignore_git_folder,
          _ => false,
        }
      })
      .unwrap_or(false)
      || self.is_vendor_folder(path)
  }

  fn is_vendor_folder(&self, path: &Path) -> bool {
    self
      .vendor_folder
      .as_ref()
      .map(|vendor_folder| path == *vendor_folder)
      .unwrap_or(false)
  }
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use sys_traits::impls::RealSys;
  use tempfile::TempDir;

  use super::*;
  use crate::glob::FilePatterns;
  use crate::glob::PathOrPattern;
  use crate::glob::PathOrPatternSet;

  #[allow(clippy::disallowed_methods)] // allow fs methods
  #[test]
  fn test_collect_files() {
    fn create_files(dir_path: &PathBuf, files: &[&str]) {
      std::fs::create_dir_all(dir_path).unwrap();
      for f in files {
        std::fs::write(dir_path.join(f), "").unwrap();
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // |   ├── git
    // |   |   └── git.js
    // |   ├── node_modules
    // |   |   └── node_modules.js
    // |   ├── vendor
    // |   |   └── vendor.js
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new().unwrap();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    std::fs::create_dir_all(t.path().join("dir.ts/child/node_modules"))
      .unwrap();
    std::fs::write(
      t.path().join("dir.ts/child/node_modules/node_modules.js"),
      "",
    )
    .unwrap();
    std::fs::create_dir_all(t.path().join("dir.ts/child/.git")).unwrap();
    std::fs::write(t.path().join("dir.ts/child/.git/git.js"), "").unwrap();
    std::fs::create_dir_all(t.path().join("dir.ts/child/vendor")).unwrap();
    std::fs::write(t.path().join("dir.ts/child/vendor/vendor.js"), "").unwrap();

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let file_patterns = FilePatterns {
      base: root_dir_path.to_path_buf(),
      include: None,
      exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
        ignore_dir_path.to_path_buf(),
      )]),
    };
    let file_collector = FileCollector::new(|e| {
      // exclude dotfiles
      e.path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|f| !f.starts_with('.'))
        .unwrap_or(false)
    });

    let result = file_collector.collect_file_patterns(&RealSys, &file_patterns);
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "git.js",
      "node_modules.js",
      "vendor.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test ignoring the .git and node_modules folder
    let file_collector = file_collector
      .ignore_git_folder()
      .ignore_node_modules()
      .set_vendor_folder(Some(child_dir_path.join("vendor").to_path_buf()));
    let result = file_collector.collect_file_patterns(&RealSys, &file_patterns);
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test opting out of ignoring by specifying the dir
    let file_patterns = FilePatterns {
      base: root_dir_path.to_path_buf(),
      include: Some(PathOrPatternSet::new(vec![
        PathOrPattern::Path(root_dir_path.to_path_buf()),
        PathOrPattern::Path(
          root_dir_path.to_path_buf().join("child/node_modules/"),
        ),
      ])),
      exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
        ignore_dir_path.to_path_buf(),
      )]),
    };
    let result = file_collector.collect_file_patterns(&RealSys, &file_patterns);
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "node_modules.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);
  }
}
