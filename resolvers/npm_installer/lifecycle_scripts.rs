// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Error as AnyError;
use deno_error::JsErrorBox;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmResolutionPackage;
use deno_semver::package::PackageNv;
use deno_semver::SmallStackString;
use deno_semver::Version;
use sys_traits::FsMetadata;

use crate::CachedNpmPackageExtraInfoProvider;
use crate::LifecycleScriptsConfig;
use crate::PackagesAllowedScripts;

pub struct PackageWithScript<'a> {
  pub package: &'a NpmResolutionPackage,
  pub scripts: HashMap<SmallStackString, String>,
  pub package_folder: PathBuf,
}

pub struct LifecycleScriptsExecutorOptions<'a> {
  pub init_cwd: &'a Path,
  pub process_state: &'a str,
  pub root_node_modules_dir_path: &'a Path,
  pub on_ran_pkg_scripts:
    &'a dyn Fn(&NpmResolutionPackage) -> Result<(), JsErrorBox>,
  pub snapshot: &'a NpmResolutionSnapshot,
  pub system_packages: &'a [NpmResolutionPackage],
  pub packages_with_scripts: &'a [PackageWithScript<'a>],
  pub extra_info_provider: &'a CachedNpmPackageExtraInfoProvider,
}

#[derive(Debug)]
pub struct NullLifecycleScriptsExecutor;

#[async_trait::async_trait(?Send)]
impl LifecycleScriptsExecutor for NullLifecycleScriptsExecutor {
  async fn execute(
    &self,
    _options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}

#[async_trait::async_trait(?Send)]
pub trait LifecycleScriptsExecutor: Sync + Send {
  async fn execute(
    &self,
    options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError>;
}

pub trait LifecycleScriptsStrategy {
  fn can_run_scripts(&self) -> bool {
    true
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> Result<(), std::io::Error>;

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool;

  fn has_run(&self, package: &NpmResolutionPackage) -> bool;
}

pub fn has_lifecycle_scripts(
  sys: &impl FsMetadata,
  extra: &NpmPackageExtraInfo,
  package_path: &Path,
) -> bool {
  if let Some(install) = extra.scripts.get("install") {
    {
      // default script
      if !is_broken_default_install_script(sys, install, package_path) {
        return true;
      }
    }
  }
  extra.scripts.contains_key("preinstall")
    || extra.scripts.contains_key("postinstall")
}

// npm defaults to running `node-gyp rebuild` if there is a `binding.gyp` file
// but it always fails if the package excludes the `binding.gyp` file when they publish.
// (for example, `fsevents` hits this)
pub fn is_broken_default_install_script(
  sys: &impl FsMetadata,
  script: &str,
  package_path: &Path,
) -> bool {
  script == "node-gyp rebuild"
    && !sys.fs_exists_no_err(package_path.join("binding.gyp"))
}

pub struct LifecycleScripts<'a, TSys: FsMetadata> {
  sys: &'a TSys,
  packages_with_scripts: Vec<PackageWithScript<'a>>,
  packages_with_scripts_not_run: Vec<(&'a NpmResolutionPackage, PathBuf)>,

  config: &'a LifecycleScriptsConfig,
  strategy: Box<dyn LifecycleScriptsStrategy + 'a>,
}

impl<'a, TSys: FsMetadata> LifecycleScripts<'a, TSys> {
  pub fn new<TLifecycleScriptsStrategy: LifecycleScriptsStrategy + 'a>(
    sys: &'a TSys,
    config: &'a LifecycleScriptsConfig,
    strategy: TLifecycleScriptsStrategy,
  ) -> Self {
    Self {
      sys,
      config,
      packages_with_scripts: Vec::new(),
      packages_with_scripts_not_run: Vec::new(),
      strategy: Box::new(strategy),
    }
  }

  pub fn can_run_scripts(&self, package_nv: &PackageNv) -> bool {
    if !self.strategy.can_run_scripts() {
      return false;
    }
    match &self.config.allowed {
      PackagesAllowedScripts::All => true,
      // TODO: make this more correct
      PackagesAllowedScripts::Some(allow_list) => allow_list.iter().any(|s| {
        let s = s.strip_prefix("npm:").unwrap_or(s);
        s == package_nv.name || s == package_nv.to_string()
      }),
      PackagesAllowedScripts::None => false,
    }
  }

  pub fn has_run_scripts(&self, package: &NpmResolutionPackage) -> bool {
    self.strategy.has_run(package)
  }

  /// Register a package for running lifecycle scripts, if applicable.
  ///
  /// `package_path` is the path containing the package's code (its root dir).
  /// `package_meta_path` is the path to serve as the base directory for lifecycle
  /// script-related metadata (e.g. to store whether the scripts have been run already)
  pub fn add(
    &mut self,
    package: &'a NpmResolutionPackage,
    extra: &NpmPackageExtraInfo,
    package_path: Cow<Path>,
  ) {
    if has_lifecycle_scripts(self.sys, extra, &package_path) {
      if self.can_run_scripts(&package.id.nv) {
        if !self.has_run_scripts(package) {
          self.packages_with_scripts.push(PackageWithScript {
            package,
            scripts: extra.scripts.clone(),
            package_folder: package_path.into_owned(),
          });
        }
      } else if !self.has_run_scripts(package)
        && (self.config.explicit_install || !self.strategy.has_warned(package))
      {
        // Skip adding `esbuild` as it is known that it can work properly without lifecycle script
        // being run, and it's also very popular - any project using Vite would raise warnings.
        {
          let nv = &package.id.nv;
          if nv.name == "esbuild"
            && nv.version >= Version::parse_standard("0.18.0").unwrap()
          {
            return;
          }
        }

        self
          .packages_with_scripts_not_run
          .push((package, package_path.into_owned()));
      }
    }
  }

  pub fn warn_not_run_scripts(&self) -> Result<(), std::io::Error> {
    if !self.packages_with_scripts_not_run.is_empty() {
      self
        .strategy
        .warn_on_scripts_not_run(&self.packages_with_scripts_not_run)?;
    }
    Ok(())
  }

  pub fn packages_with_scripts(&self) -> &[PackageWithScript<'a>] {
    &self.packages_with_scripts
  }
}

pub static LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR: &str =
  "DENO_INTERNAL_IS_LIFECYCLE_SCRIPT";

pub fn is_running_lifecycle_script(sys: &impl sys_traits::EnvVar) -> bool {
  sys.env_var(LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR).is_ok()
}
