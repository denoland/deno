// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::bin_entries::BinEntries;
use crate::args::LifecycleScriptsConfig;
use deno_core::anyhow::Context;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_runtime::deno_io::FromRawIoHandle;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use std::borrow::Cow;
use std::rc::Rc;

use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_npm::NpmResolutionPackage;

pub trait LifecycleScriptsStrategy {
  fn can_run_scripts(&self) -> bool {
    true
  }
  fn package_path(&self, package: &NpmResolutionPackage) -> PathBuf;

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> Result<(), AnyError>;

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool;

  fn has_run(&self, package: &NpmResolutionPackage) -> bool;

  fn did_run_scripts(
    &self,
    package: &NpmResolutionPackage,
  ) -> Result<(), AnyError>;
}

pub struct LifecycleScripts<'a> {
  packages_with_scripts: Vec<(&'a NpmResolutionPackage, PathBuf)>,
  packages_with_scripts_not_run: Vec<(&'a NpmResolutionPackage, PathBuf)>,

  config: &'a LifecycleScriptsConfig,
  strategy: Box<dyn LifecycleScriptsStrategy + 'a>,
}

impl<'a> LifecycleScripts<'a> {
  pub fn new<T: LifecycleScriptsStrategy + 'a>(
    config: &'a LifecycleScriptsConfig,
    strategy: T,
  ) -> Self {
    Self {
      config,
      packages_with_scripts: Vec::new(),
      packages_with_scripts_not_run: Vec::new(),
      strategy: Box::new(strategy),
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

impl<'a> LifecycleScripts<'a> {
  fn can_run_scripts(&self, package_nv: &PackageNv) -> bool {
    if !self.strategy.can_run_scripts() {
      return false;
    }
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
  ) {
    if has_lifecycle_scripts(package, &package_path) {
      if self.can_run_scripts(&package.id.nv) {
        if !self.strategy.has_run(package) {
          self
            .packages_with_scripts
            .push((package, package_path.into_owned()));
        }
      } else if !self.strategy.has_run(package)
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

  pub fn warn_not_run_scripts(&self) -> Result<(), AnyError> {
    if !self.packages_with_scripts_not_run.is_empty() {
      self
        .strategy
        .warn_on_scripts_not_run(&self.packages_with_scripts_not_run)?;
    }
    Ok(())
  }

  pub async fn finish(
    self,
    snapshot: &NpmResolutionSnapshot,
    packages: &[NpmResolutionPackage],
    root_node_modules_dir_path: Option<&Path>,
  ) -> Result<(), AnyError> {
    self.warn_not_run_scripts()?;
    let get_package_path =
      |p: &NpmResolutionPackage| self.strategy.package_path(p);
    let mut failed_packages = Vec::new();
    if !self.packages_with_scripts.is_empty() {
      // get custom commands for each bin available in the node_modules dir (essentially
      // the scripts that are in `node_modules/.bin`)
      let base =
        resolve_baseline_custom_commands(snapshot, packages, get_package_path)?;
      let init_cwd = &self.config.initial_cwd;
      let process_state = crate::npm::managed::npm_process_state(
        snapshot.as_valid_serialized(),
        root_node_modules_dir_path,
      );

      let mut env_vars = crate::task_runner::real_env_vars();
      // we want to pass the current state of npm resolution down to the deno subprocess
      // (that may be running as part of the script). we do this with an inherited temp file
      //
      // SAFETY: we are sharing a single temp file across all of the scripts. the file position
      // will be shared among these, which is okay since we run only one script at a time.
      // However, if we concurrently run scripts in the future we will
      // have to have multiple temp files.
      let temp_file_fd =
        deno_runtime::ops::process::npm_process_state_tempfile(
          process_state.as_bytes(),
        ).context("failed to create npm process state tempfile for running lifecycle scripts")?;
      // SAFETY: fd/handle is valid
      let _temp_file =
        unsafe { std::fs::File::from_raw_io_handle(temp_file_fd) }; // make sure the file gets closed
      env_vars.insert(
        deno_runtime::ops::process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME
          .to_string(),
        (temp_file_fd as usize).to_string(),
      );
      for (package, package_path) in self.packages_with_scripts {
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
        self.strategy.did_run_scripts(package)?;
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
