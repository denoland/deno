// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod common;
mod global;
mod local;

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::url::Url;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;

use crate::util::progress_bar::ProgressBar;

pub use self::common::NpmPackageFsResolver;

use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;

use super::cache::NpmCache;
use super::resolution::NpmResolution;

pub fn create_npm_fs_resolver(
  fs: Arc<dyn FileSystem>,
  cache: Arc<NpmCache>,
  progress_bar: &ProgressBar,
  registry_url: Url,
  resolution: Arc<NpmResolution>,
  maybe_node_modules_path: Option<PathBuf>,
  system_info: NpmSystemInfo,
) -> Arc<dyn NpmPackageFsResolver> {
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
      fs,
      cache,
      progress_bar.clone(),
      registry_url,
      node_modules_folder,
      resolution,
      system_info,
    )),
    None => Arc::new(GlobalNpmPackageResolver::new(
      fs,
      cache,
      registry_url,
      resolution,
      system_info,
    )),
  }
}
