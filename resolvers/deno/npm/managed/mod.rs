// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::PathBuf;
use std::sync::Arc;

use deno_npm_cache::NpmCache;
pub use resolution::NpmResolution;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;

pub use self::common::NpmPackageFsResolver;
pub use self::common::NpmPackageFsResolverPackageFolderError;
use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;

pub fn create_npm_fs_resolver<
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + FsCanonicalize
    + Send
    + Sync
    + 'static,
>(
  npm_cache: Arc<NpmCache<TSys>>,
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
    None => Arc::new(GlobalNpmPackageResolver::new(npm_cache, resolution)),
  }
}
