// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod bin_entries;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::StreamExt;
use deno_core::url::Url;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use deno_semver::package::PackageNv;
use node_resolver::errors::PackageFolderResolveError;

use crate::args::LifecycleScriptsConfig;
use crate::npm::managed::cache::TarballCache;
use bin_entries::BinEntries;

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsResolver: Send + Sync {
  /// Specifier for the root directory.
  fn root_dir_url(&self) -> &Url;

  /// The local node_modules folder if it is applicable to the implementation.
  fn node_modules_path(&self) -> Option<&PathBuf>;

  fn maybe_package_folder(&self, package_id: &NpmPackageId) -> Option<PathBuf>;

  fn package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, AnyError> {
    self.maybe_package_folder(package_id).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "Package folder not found for '{}'",
        package_id.as_serialized()
      )
    })
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError>;

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageCacheFolderId>, AnyError>;

  async fn cache_packages(&self) -> Result<(), AnyError>;

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError>;
}

#[derive(Debug)]
pub struct RegistryReadPermissionChecker {
  fs: Arc<dyn FileSystem>,
  cache: Mutex<HashMap<PathBuf, PathBuf>>,
  registry_path: PathBuf,
}

impl RegistryReadPermissionChecker {
  pub fn new(fs: Arc<dyn FileSystem>, registry_path: PathBuf) -> Self {
    Self {
      fs,
      registry_path,
      cache: Default::default(),
    }
  }

  pub fn ensure_registry_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    // allow reading if it's in the node_modules
    let is_path_in_node_modules = path.starts_with(&self.registry_path)
      && path
        .components()
        .all(|c| !matches!(c, std::path::Component::ParentDir));

    if is_path_in_node_modules {
      let mut cache = self.cache.lock().unwrap();
      let mut canonicalize =
        |path: &Path| -> Result<Option<PathBuf>, AnyError> {
          match cache.get(path) {
            Some(canon) => Ok(Some(canon.clone())),
            None => match self.fs.realpath_sync(path) {
              Ok(canon) => {
                cache.insert(path.to_path_buf(), canon.clone());
                Ok(Some(canon))
              }
              Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                  return Ok(None);
                }
                Err(AnyError::from(e)).with_context(|| {
                  format!("failed canonicalizing '{}'", path.display())
                })
              }
            },
          }
        };
      let Some(registry_path_canon) = canonicalize(&self.registry_path)? else {
        return Ok(()); // not exists, allow reading
      };
      let Some(path_canon) = canonicalize(path)? else {
        return Ok(()); // not exists, allow reading
      };

      if path_canon.starts_with(registry_path_canon) {
        return Ok(());
      }
    }

    _ = permissions.check_read_path(path)?;
    Ok(())
  }
}

/// Caches all the packages in parallel.
pub async fn cache_packages(
  packages: &[NpmResolutionPackage],
  tarball_cache: &Arc<TarballCache>,
) -> Result<(), AnyError> {
  let mut futures_unordered = futures::stream::FuturesUnordered::new();
  for package in packages {
    futures_unordered.push(async move {
      tarball_cache
        .ensure_package(&package.id.nv, &package.dist)
        .await
    });
  }
  while let Some(result) = futures_unordered.next().await {
    // surface the first error
    result?;
  }
  Ok(())
}

pub struct LifecycleScripts<'a> {
  packages_with_scripts: Vec<(&'a NpmResolutionPackage, PathBuf, PathBuf)>,
  packages_with_scripts_not_run: Vec<(PathBuf, &'a PackageNv)>,
  config: &'a LifecycleScriptsConfig,
}

impl<'a> LifecycleScripts<'a> {
  pub fn new(config: &'a LifecycleScriptsConfig) -> Self {
    Self {
      config,
      packages_with_scripts: Vec::new(),
      packages_with_scripts_not_run: Vec::new(),
    }
  }
}

fn has_lifecycle_scripts(
  package: &NpmResolutionPackage,
  package_path: &Path,
) -> bool {
  if let Some(install) = package.scripts.get("install") {
    // default script
    if !is_broken_default_install_script(install, package_path) {
      return true;
    }
  }
  package.scripts.contains_key("preinstall")
    || package.scripts.contains_key("postinstall")
}

// npm defaults to running `node-gyp rebuild` if there is a `binding.gyp` file
// but it always fails if the package excludes the `binding.gyp` file when they publish.
// (for example, `fsevents` hits this)
fn is_broken_default_install_script(script: &str, package_path: &Path) -> bool {
  script == "node-gyp rebuild" && !package_path.join("binding.gyp").exists()
}

pub fn default_warn_not_run(
  packages_with_scripts_not_run: &[(PathBuf, &PackageNv)],
) {
  if !packages_with_scripts_not_run.is_empty() {
    let (maybe_install, maybe_install_example) = if *crate::args::DENO_FUTURE {
      (
        " or `deno install`",
        " or `deno install --allow-scripts=pkg1,pkg2`",
      )
    } else {
      ("", "")
    };
    let packages = packages_with_scripts_not_run
      .iter()
      .map(|(_, p)| format!("npm:{p}"))
      .collect::<Vec<_>>()
      .join(", ");
    log::warn!("{}: Packages contained npm lifecycle scripts (preinstall/install/postinstall) that were not executed.
    This may cause the packages to not work correctly. To run them, use the `--allow-scripts` flag with `deno cache`{maybe_install}
    (e.g. `deno cache --allow-scripts=pkg1,pkg2 <entrypoint>`{maybe_install_example}):\n      {packages}", crate::colors::yellow("warning"));
  }
}

impl<'a> LifecycleScripts<'a> {
  fn can_run_scripts(&self, package_nv: &PackageNv) -> bool {
    use crate::args::PackagesAllowedScripts;
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
  /// Register a package for running lifecycle scripts, if applicable.
  ///
  /// `package_path` is the path containing the package's code (its root dir).
  /// `package_meta_path` is the path to serve as the base directory for lifecycle
  /// script-related metadata (e.g. to store whether the scripts have been run already)
  pub fn add(
    &mut self,
    package: &'a NpmResolutionPackage,
    package_path: Cow<Path>,
    package_meta_path: &Path,
  ) {
    if has_lifecycle_scripts(package, &package_path) {
      let scripts_run = package_meta_path.join(".scripts-run");
      let has_warned = package_meta_path.join(".scripts-warned");
      if self.can_run_scripts(&package.id.nv) {
        if !scripts_run.exists() {
          self.packages_with_scripts.push((
            package,
            package_path.into_owned(),
            scripts_run,
          ));
        }
      } else if !scripts_run.exists() && !has_warned.exists() {
        self
          .packages_with_scripts_not_run
          .push((has_warned, &package.id.nv));
      }
    }
  }

  pub fn will_run_scripts(&self) -> bool {
    !self.packages_with_scripts.is_empty()
  }

  pub fn warn_not_run_scripts(
    &self,
    warn_fn: impl Fn(&[(PathBuf, &PackageNv)]),
  ) {
    if !self.packages_with_scripts_not_run.is_empty() {
      warn_fn(&self.packages_with_scripts_not_run);
      for (scripts_warned_path, _) in &self.packages_with_scripts_not_run {
        let _ignore_err = std::fs::write(scripts_warned_path, "");
      }
    }
  }

  pub async fn finish(
    self,
    snapshot: &NpmResolutionSnapshot,
    packages: &[NpmResolutionPackage],
    root_node_modules_dir_path: Option<&Path>,
    get_package_path: impl Fn(&NpmResolutionPackage) -> PathBuf + Copy,
    warn_fn: impl Fn(&[(PathBuf, &PackageNv)]),
  ) -> Result<(), AnyError> {
    self.warn_not_run_scripts(warn_fn);
    let mut failed_packages = Vec::new();
    if !self.packages_with_scripts.is_empty() {
      // get custom commands for each bin available in the node_modules dir (essentially
      // the scripts that are in `node_modules/.bin`)
      let base =
        resolve_baseline_custom_commands(snapshot, packages, get_package_path)?;
      let init_cwd = self.config.initial_cwd.as_deref().unwrap();
      let process_state = crate::npm::managed::npm_process_state(
        snapshot.as_valid_serialized(),
        root_node_modules_dir_path,
      );

      let mut env_vars = crate::task_runner::real_env_vars();
      env_vars.insert(
        crate::args::NPM_RESOLUTION_STATE_ENV_VAR_NAME.to_string(),
        process_state,
      );
      for (package, package_path, scripts_run_path) in
        self.packages_with_scripts
      {
        // add custom commands for binaries from the package's dependencies. this will take precedence over the
        // baseline commands, so if the package relies on a bin that conflicts with one higher in the dependency tree, the
        // correct bin will be used.
        let custom_commands = resolve_custom_commands_from_deps(
          base.clone(),
          package,
          snapshot,
          get_package_path,
        )?;
        for script_name in ["preinstall", "install", "postinstall"] {
          if let Some(script) = package.scripts.get(script_name) {
            if script_name == "install"
              && is_broken_default_install_script(script, &package_path)
            {
              continue;
            }
            let exit_code = crate::task_runner::run_task(
              crate::task_runner::RunTaskOptions {
                task_name: script_name,
                script,
                cwd: &package_path,
                env_vars: env_vars.clone(),
                custom_commands: custom_commands.clone(),
                init_cwd,
                argv: &[],
                root_node_modules_dir: root_node_modules_dir_path,
              },
            )
            .await?;
            if exit_code != 0 {
              log::warn!(
                "error: script '{}' in '{}' failed with exit code {}",
                script_name,
                package.id.nv,
                exit_code,
              );
              failed_packages.push(&package.id.nv);
              // assume if earlier script fails, later ones will fail too
              break;
            }
          }
        }
        std::fs::write(scripts_run_path, "")?;
      }
    }
    if failed_packages.is_empty() {
      Ok(())
    } else {
      Err(AnyError::msg(format!(
        "failed to run scripts for packages: {}",
        failed_packages
          .iter()
          .map(|p| p.to_string())
          .collect::<Vec<_>>()
          .join(", ")
      )))
    }
  }
}

// take in all (non copy) packages from snapshot,
// and resolve the set of available binaries to create
// custom commands available to the task runner
fn resolve_baseline_custom_commands(
  snapshot: &NpmResolutionSnapshot,
  packages: &[NpmResolutionPackage],
  get_package_path: impl Fn(&NpmResolutionPackage) -> PathBuf,
) -> Result<crate::task_runner::TaskCustomCommands, AnyError> {
  let mut custom_commands = crate::task_runner::TaskCustomCommands::new();
  custom_commands
    .insert("npx".to_string(), Rc::new(crate::task_runner::NpxCommand));

  custom_commands
    .insert("npm".to_string(), Rc::new(crate::task_runner::NpmCommand));

  custom_commands
    .insert("node".to_string(), Rc::new(crate::task_runner::NodeCommand));

  custom_commands.insert(
    "node-gyp".to_string(),
    Rc::new(crate::task_runner::NodeGypCommand),
  );

  // TODO: this recreates the bin entries which could be redoing some work, but the ones
  // we compute earlier in `sync_resolution_with_fs` may not be exhaustive (because we skip
  // doing it for packages that are set up already.
  // realistically, scripts won't be run very often so it probably isn't too big of an issue.
  resolve_custom_commands_from_packages(
    custom_commands,
    snapshot,
    packages,
    get_package_path,
  )
}

// resolves the custom commands from an iterator of packages
// and adds them to the existing custom commands.
// note that this will overwrite any existing custom commands
fn resolve_custom_commands_from_packages<
  'a,
  P: IntoIterator<Item = &'a NpmResolutionPackage>,
>(
  mut commands: crate::task_runner::TaskCustomCommands,
  snapshot: &'a NpmResolutionSnapshot,
  packages: P,
  get_package_path: impl Fn(&'a NpmResolutionPackage) -> PathBuf,
) -> Result<crate::task_runner::TaskCustomCommands, AnyError> {
  let mut bin_entries = BinEntries::new();
  for package in packages {
    let package_path = get_package_path(package);

    if package.bin.is_some() {
      bin_entries.add(package, package_path);
    }
  }
  let bins = bin_entries.into_bin_files(snapshot);
  for (bin_name, script_path) in bins {
    commands.insert(
      bin_name.clone(),
      Rc::new(crate::task_runner::NodeModulesFileRunCommand {
        command_name: bin_name,
        path: script_path,
      }),
    );
  }

  Ok(commands)
}

// resolves the custom commands from the dependencies of a package
// and adds them to the existing custom commands.
// note that this will overwrite any existing custom commands.
fn resolve_custom_commands_from_deps(
  baseline: crate::task_runner::TaskCustomCommands,
  package: &NpmResolutionPackage,
  snapshot: &NpmResolutionSnapshot,
  get_package_path: impl Fn(&NpmResolutionPackage) -> PathBuf,
) -> Result<crate::task_runner::TaskCustomCommands, AnyError> {
  resolve_custom_commands_from_packages(
    baseline,
    snapshot,
    package
      .dependencies
      .values()
      .map(|id| snapshot.package_from_id(id).unwrap()),
    get_package_path,
  )
}
