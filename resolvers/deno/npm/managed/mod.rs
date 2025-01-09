// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::npm::NpmCacheDir;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;

pub use self::common::NpmPackageFsResolver;
pub use self::common::NpmPackageFsResolverPackageFolderError;
use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;
pub use self::resolution::NpmResolution;

pub fn create_npm_fs_resolver<
  TSys: FsCanonicalize + FsMetadata + Send + Sync + 'static,
>(
  npm_cache_dir: &Arc<NpmCacheDir>,
  npm_rc: &Arc<deno_npm::npm_rc::ResolvedNpmRc>,
  resolution: Arc<NpmResolution>,
  sys: TSys,
  maybe_node_modules_path: Option<PathBuf>,
) -> Arc<dyn NpmPackageFsResolver> {
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
      resolution,
      sys,
      node_modules_folder,
    )),
    None => Arc::new(GlobalNpmPackageResolver::new(
      npm_cache_dir.clone(),
      npm_rc.clone(),
      resolution,
    )),
  }
}
