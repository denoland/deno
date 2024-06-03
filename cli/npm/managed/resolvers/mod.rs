// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod common;
mod global;
mod local;

use std::path::PathBuf;
use std::sync::Arc;

use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;

use crate::util::progress_bar::ProgressBar;

pub use self::common::NpmPackageFsResolver;

use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;

use super::cache::NpmCache;
use super::cache::TarballCache;
use super::resolution::NpmResolution;

pub fn create_npm_fs_resolver(
  fs: Arc<dyn FileSystem>,
  npm_cache: Arc<NpmCache>,
  npm_rc: Arc<ResolvedNpmRc>,
  progress_bar: &ProgressBar,
  resolution: Arc<NpmResolution>,
  maybe_node_modules_path: Option<PathBuf>,
  system_info: NpmSystemInfo,
) -> Arc<dyn NpmPackageFsResolver> {
  let tarball_cache = Arc::new(TarballCache::new(
    npm_cache.clone(),
    fs.clone(),
    npm_rc,
    progress_bar.clone(),
  ));
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
      npm_cache,
      fs,
      progress_bar.clone(),
      resolution,
      tarball_cache,
      node_modules_folder,
      system_info,
    )),
    None => Arc::new(GlobalNpmPackageResolver::new(
      npm_cache,
      fs,
      tarball_cache,
      resolution,
      system_info,
    )),
  }
}
