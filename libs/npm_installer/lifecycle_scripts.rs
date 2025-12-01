// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Error as AnyError;
use deno_error::JsErrorBox;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmResolutionPackage;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_semver::SmallStackString;
use deno_semver::Version;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
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

pub struct LifecycleScriptsWarning {
  message: String,

  did_warn_fn: DidWarnFn,
}

impl std::fmt::Debug for LifecycleScriptsWarning {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LifecycleScriptsWarning")
      .field("message", &self.message)
      .finish()
  }
}

type DidWarnFn =
  Box<dyn FnOnce(&dyn sys_traits::boxed::FsOpenBoxed) + Send + Sync>;

impl LifecycleScriptsWarning {
  pub(crate) fn new(message: String, did_warn_fn: DidWarnFn) -> Self {
    Self {
      message,
      did_warn_fn,
    }
  }

  pub fn into_message(
    self,
    sys: &dyn sys_traits::boxed::FsOpenBoxed,
  ) -> String {
    (self.did_warn_fn)(sys);
    self.message
  }
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
    fn matches_nv(req: &PackageReq, package_nv: &PackageNv) -> bool {
      // we shouldn't support this being a tag because it's too complicated
      debug_assert!(req.version_req.tag().is_none());
      package_nv.name == req.name
        && req.version_req.matches(&package_nv.version)
    }

    if !self.strategy.can_run_scripts() {
      return false;
    }
    let matches_allowed = match &self.config.allowed {
      PackagesAllowedScripts::All => true,
      PackagesAllowedScripts::Some(allow_list) => {
        allow_list.iter().any(|req| matches_nv(req, package_nv))
      }
      PackagesAllowedScripts::None => false,
    };
    matches_allowed
      && !self
        .config
        .denied
        .iter()
        .any(|req| matches_nv(req, package_nv))
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
    package_path: Cow<'_, Path>,
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
        && !(self.config.denied.iter().any(|d| {
          package.id.nv.name == d.name
            && d.version_req.matches(&package.id.nv.version)
        }))
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
