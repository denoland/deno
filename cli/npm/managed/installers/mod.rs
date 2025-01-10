// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_npm::NpmSystemInfo;
use deno_resolver::npm::managed::NpmResolution;

pub use self::common::NpmPackageFsInstaller;
use self::global::GlobalNpmPackageInstaller;
use self::local::LocalNpmPackageInstaller;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmInstallDepsProvider;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmTarballCache;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

mod common;
mod global;
mod local;

#[allow(clippy::too_many_arguments)]
pub fn create_npm_fs_installer(
  npm_cache: Arc<CliNpmCache>,
  npm_install_deps_provider: &Arc<NpmInstallDepsProvider>,
  progress_bar: &ProgressBar,
  resolution: Arc<NpmResolution>,
  sys: CliSys,
  tarball_cache: Arc<CliNpmTarballCache>,
  maybe_node_modules_path: Option<PathBuf>,
  system_info: NpmSystemInfo,
  lifecycle_scripts: LifecycleScriptsConfig,
) -> Arc<dyn NpmPackageFsInstaller> {
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageInstaller::new(
      npm_cache,
      npm_install_deps_provider.clone(),
      progress_bar.clone(),
      resolution,
      sys,
      tarball_cache,
      node_modules_folder,
      system_info,
      lifecycle_scripts,
    )),
    None => Arc::new(GlobalNpmPackageInstaller::new(
      npm_cache,
      tarball_cache,
      resolution,
      system_info,
      lifecycle_scripts,
    )),
  }
}
