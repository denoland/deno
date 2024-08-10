// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use crate::sync::MaybeSend;
use crate::sync::MaybeSync;

pub struct NodeResolverFsStat {
  pub is_file: bool,
  pub is_dir: bool,
  pub is_symlink: bool,
}

pub trait NodeResolverEnv: std::fmt::Debug + MaybeSend + MaybeSync {
  fn is_builtin_node_module(&self, specifier: &str) -> bool;

  fn realpath_sync(&self, path: &Path) -> std::io::Result<PathBuf>;

  fn stat_sync(&self, path: &Path) -> std::io::Result<NodeResolverFsStat>;

  fn exists_sync(&self, path: &Path) -> bool;

  fn is_file_sync(&self, path: &Path) -> bool {
    self
      .stat_sync(path)
      .map(|stat| stat.is_file)
      .unwrap_or(false)
  }

  fn is_dir_sync(&self, path: &Path) -> bool {
    self
      .stat_sync(path)
      .map(|stat| stat.is_dir)
      .unwrap_or(false)
  }

  fn pkg_json_fs(&self) -> &dyn deno_package_json::fs::DenoPkgJsonFs;
}
