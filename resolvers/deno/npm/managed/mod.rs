// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::PathBuf;

use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;

pub use self::common::NpmPackageFsResolver;
pub use self::common::NpmPackageFsResolverPackageFolderError;
use self::common::NpmPackageFsResolverRc;
use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;
pub use self::resolution::NpmResolution;
use self::resolution::NpmResolutionRc;
use crate::sync::new_rc;
use crate::NpmCacheDirRc;
use crate::ResolvedNpmRcRc;

pub fn create_npm_fs_resolver<
  TSys: FsCanonicalize + FsMetadata + Send + Sync + 'static,
>(
  npm_cache_dir: &NpmCacheDirRc,
  npm_rc: &ResolvedNpmRcRc,
  resolution: NpmResolutionRc,
  sys: TSys,
  maybe_node_modules_path: Option<PathBuf>,
) -> NpmPackageFsResolverRc {
  match maybe_node_modules_path {
    Some(node_modules_folder) => new_rc(LocalNpmPackageResolver::new(
      resolution,
      sys,
      node_modules_folder,
    )),
    None => new_rc(GlobalNpmPackageResolver::new(
      npm_cache_dir.clone(),
      npm_rc.clone(),
      resolution,
    )),
  }
}
