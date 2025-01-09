// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::Path;
use std::path::PathBuf;

use node_resolver::InNpmPackageChecker;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

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
  TSys: FsCanonicalize + FsMetadata + std::fmt::Debug + Send + Sync + 'static,
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

#[derive(Debug)]
pub struct ManagedInNpmPackageChecker {
  root_dir: Url,
}

impl InNpmPackageChecker for ManagedInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.as_ref().starts_with(self.root_dir.as_str())
  }
}

pub struct ManagedInNpmPkgCheckerCreateOptions<'a> {
  pub root_cache_dir_url: &'a Url,
  pub maybe_node_modules_path: Option<&'a Path>,
}

pub fn create_managed_in_npm_pkg_checker(
  options: ManagedInNpmPkgCheckerCreateOptions,
) -> ManagedInNpmPackageChecker {
  let root_dir = match options.maybe_node_modules_path {
    Some(node_modules_folder) => {
      deno_path_util::url_from_directory_path(node_modules_folder).unwrap()
    }
    None => options.root_cache_dir_url.clone(),
  };
  debug_assert!(root_dir.as_str().ends_with('/'));
  ManagedInNpmPackageChecker { root_dir }
}
