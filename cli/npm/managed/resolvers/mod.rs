// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;

use std::path::PathBuf;
use std::sync::Arc;

pub use self::common::NpmPackageFsResolver;
pub use self::common::NpmPackageFsResolverPackageFolderError;
use self::global::GlobalNpmPackageResolver;
pub use self::local::get_package_folder_id_folder_name;
use self::local::LocalNpmPackageResolver;
use super::resolution::NpmResolution;
use crate::npm::CliNpmCache;
use crate::sys::CliSys;

#[allow(clippy::too_many_arguments)]
pub fn create_npm_fs_resolver(
  npm_cache: Arc<CliNpmCache>,
  resolution: Arc<NpmResolution>,
  sys: CliSys,
  maybe_node_modules_path: Option<PathBuf>,
) -> Arc<dyn NpmPackageFsResolver> {
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
      resolution,
      sys,
      node_modules_folder,
    )),
    None => Arc::new(GlobalNpmPackageResolver::new(npm_cache, resolution)),
  }
}
