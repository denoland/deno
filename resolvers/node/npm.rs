// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_path_util::url_from_directory_path;
use deno_path_util::url_from_file_path;
use url::Url;

use crate::errors;
use crate::path::PathClean;
use crate::sync::MaybeSend;
use crate::sync::MaybeSync;

#[allow(clippy::disallowed_types)]
pub type NpmResolverRc = crate::sync::MaybeArc<dyn NpmResolver>;

pub trait NpmResolver: std::fmt::Debug + MaybeSend + MaybeSync {
  /// Resolves an npm package folder path from an npm package referrer.
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &Url,
  ) -> Result<PathBuf, errors::PackageFolderResolveError>;

  fn in_npm_package(&self, specifier: &Url) -> bool;

  fn in_npm_package_at_dir_path(&self, path: &Path) -> bool {
    let specifier = match url_from_directory_path(&path.to_path_buf().clean()) {
      Ok(p) => p,
      Err(_) => return false,
    };
    self.in_npm_package(&specifier)
  }

  fn in_npm_package_at_file_path(&self, path: &Path) -> bool {
    let specifier = match url_from_file_path(&path.to_path_buf().clean()) {
      Ok(p) => p,
      Err(_) => return false,
    };
    self.in_npm_package(&specifier)
  }
}
