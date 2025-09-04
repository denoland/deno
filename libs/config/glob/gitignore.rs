// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use sys_traits::FsMetadata;
use sys_traits::FsRead;

/// Resolved gitignore for a directory.
pub struct DirGitIgnores {
  current: Option<Rc<ignore::gitignore::Gitignore>>,
  parent: Option<Rc<DirGitIgnores>>,
}

impl DirGitIgnores {
  pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
    let mut is_ignored = false;
    if let Some(parent) = &self.parent {
      is_ignored = parent.is_ignored(path, is_dir);
    }
    if let Some(current) = &self.current {
      match current.matched(path, is_dir) {
        ignore::Match::None => {}
        ignore::Match::Ignore(_) => {
          is_ignored = true;
        }
        ignore::Match::Whitelist(_) => {
          is_ignored = false;
        }
      }
    }
    is_ignored
  }
}

/// Resolves gitignores in a directory tree taking into account
/// ancestor gitignores that may be found in a directory.
pub struct GitIgnoreTree<'a, Sys: FsRead + FsMetadata> {
  sys: &'a Sys,
  ignores: HashMap<PathBuf, Option<Rc<DirGitIgnores>>>,
  include_paths: Vec<PathBuf>,
}

impl<'a, Sys: FsRead + FsMetadata> GitIgnoreTree<'a, Sys> {
  pub fn new(
    sys: &'a Sys,
    // paths that should override what's in the gitignore
    include_paths: Vec<PathBuf>,
  ) -> Self {
    Self {
      sys,
      ignores: Default::default(),
      include_paths,
    }
  }

  pub fn get_resolved_git_ignore_for_dir(
    &mut self,
    dir_path: &Path,
  ) -> Option<Rc<DirGitIgnores>> {
    // for directories, provide itself in order to tell
    // if it should stop searching for gitignores because
    // maybe this dir_path is a .git directory
    let parent = dir_path.parent()?;
    self.get_resolved_git_ignore_inner(parent, Some(dir_path))
  }

  pub fn get_resolved_git_ignore_for_file(
    &mut self,
    file_path: &Path,
  ) -> Option<Rc<DirGitIgnores>> {
    let dir_path = file_path.parent()?;
    self.get_resolved_git_ignore_inner(dir_path, None)
  }

  fn get_resolved_git_ignore_inner(
    &mut self,
    dir_path: &Path,
    maybe_parent: Option<&Path>,
  ) -> Option<Rc<DirGitIgnores>> {
    let maybe_resolved = self.ignores.get(dir_path).cloned();
    if let Some(resolved) = maybe_resolved {
      resolved
    } else {
      let resolved = self.resolve_gitignore_in_dir(dir_path, maybe_parent);
      self.ignores.insert(dir_path.to_owned(), resolved.clone());
      resolved
    }
  }

  fn resolve_gitignore_in_dir(
    &mut self,
    dir_path: &Path,
    maybe_parent: Option<&Path>,
  ) -> Option<Rc<DirGitIgnores>> {
    if let Some(parent) = maybe_parent {
      // stop searching if the parent dir had a .git directory in it
      if self.sys.fs_exists_no_err(parent.join(".git")) {
        return None;
      }
    }

    let parent = dir_path.parent().and_then(|parent| {
      self.get_resolved_git_ignore_inner(parent, Some(dir_path))
    });
    let current = self
      .sys
      .fs_read_to_string_lossy(dir_path.join(".gitignore"))
      .ok()
      .and_then(|text| {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(dir_path);
        for line in text.lines() {
          builder.add_line(None, line).ok()?;
        }
        // override the gitignore contents to include these paths
        for path in &self.include_paths {
          if let Ok(suffix) = path.strip_prefix(dir_path) {
            let suffix = suffix.to_string_lossy().replace('\\', "/");
            let _ignore = builder.add_line(None, &format!("!/{}", suffix));
            if !suffix.ends_with('/') {
              let _ignore = builder.add_line(None, &format!("!/{}/", suffix));
            }
          }
        }
        let gitignore = builder.build().ok()?;
        Some(Rc::new(gitignore))
      });
    if parent.is_none() && current.is_none() {
      None
    } else {
      Some(Rc::new(DirGitIgnores { current, parent }))
    }
  }
}

#[cfg(test)]
mod test {
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsWrite;
  use sys_traits::impls::InMemorySys;

  use super::*;

  #[test]
  fn git_ignore_tree() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/sub_dir/sub_dir").unwrap();
    sys.fs_write("/.gitignore", "file.txt").unwrap();
    sys.fs_write("/sub_dir/.gitignore", "data.txt").unwrap();
    sys
      .fs_write("/sub_dir/sub_dir/.gitignore", "!file.txt\nignore.txt")
      .unwrap();
    let mut ignore_tree = GitIgnoreTree::new(&sys, Vec::new());
    let mut run_test = |path: &str, expected: bool| {
      let path = PathBuf::from(path);
      let gitignore =
        ignore_tree.get_resolved_git_ignore_for_file(&path).unwrap();
      assert_eq!(
        gitignore.is_ignored(&path, /* is_dir */ false),
        expected,
        "Path: {}",
        path.display()
      );
    };
    run_test("/file.txt", true);
    run_test("/other.txt", false);
    run_test("/data.txt", false);
    run_test("/sub_dir/file.txt", true);
    run_test("/sub_dir/other.txt", false);
    run_test("/sub_dir/data.txt", true);
    run_test("/sub_dir/sub_dir/file.txt", false); // unignored up here
    run_test("/sub_dir/sub_dir/sub_dir/file.txt", false);
    run_test("/sub_dir/sub_dir/sub_dir/ignore.txt", true);
    run_test("/sub_dir/sub_dir/ignore.txt", true);
    run_test("/sub_dir/ignore.txt", false);
    run_test("/ignore.txt", false);
  }
}
