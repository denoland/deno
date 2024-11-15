// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

pub struct DirEntry {
  pub name: String,
  pub is_file: bool,
  pub is_directory: bool,
}

pub trait DenoResolverFs {
  fn read_to_string_lossy(&self, path: &Path) -> std::io::Result<String>;
  fn realpath_sync(&self, path: &Path) -> std::io::Result<PathBuf>;
  fn exists_sync(&self, path: &Path) -> bool;
  fn is_dir_sync(&self, path: &Path) -> bool;
  fn read_dir_sync(&self, dir_path: &Path) -> std::io::Result<Vec<DirEntry>>;
}
