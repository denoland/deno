// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::AnyError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmResolutionPackage;
use deno_resolver::npm::ManagedNpmResolverRc;
use deno_runtime::deno_io::FromRawIoHandle;
use deno_task_shell::KillSignal;

use super::bin_entries::BinEntries;
use super::lifecycle_scripts::is_broken_default_install_script;
use super::lifecycle_scripts::LifecycleScriptsExecutor;
use super::lifecycle_scripts::LifecycleScriptsExecutorOptions;
use super::lifecycle_scripts::PackageWithScript;
use super::lifecycle_scripts::LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR;
use super::CachedNpmPackageExtraInfoProvider;
use super::ExpectedExtraInfo;
use crate::sys::CliSys;
use crate::task_runner::TaskStdio;
use crate::util::progress_bar::ProgressMessagePrompt;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum DenoTaskLifecycleScriptsError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  BinEntries(#[from] super::bin_entries::BinEntriesError),
  #[class(inherit)]
  #[error(
    "failed to create npm process state tempfile for running lifecycle scripts"
  )]
  CreateNpmProcessState(#[source] std::io::Error),
  #[class(generic)]
  #[error(transparent)]
  Task(AnyError),
  #[class(generic)]
  #[error("failed to run scripts for packages: {}", .0.join(", "))]
  RunScripts(Vec<String>),
}

pub struct DenoTaskLifeCycleScriptsExecutor {
  npm_resolver: ManagedNpmResolverRc<CliSys>,
}

#[async_trait::async_trait(?Send)]
impl LifecycleScriptsExecutor for DenoTaskLifeCycleScriptsExecutor {
  async fn execute(
    &self,
    options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError> {
    let mut failed_packages = Vec::new();
    let mut bin_entries = BinEntries::new();
    // get custom commands for each bin available in the node_modules dir (essentially
    // the scripts that are in `node_modules/.bin`)
    let base = self
      .resolve_baseline_custom_commands(
        options.extra_info_provider,
        &mut bin_entries,
        options.snapshot,
        options.system_packages,
      )
      .await;

    // we don't run with signals forwarded because once signals
    // are setup then they're process wide.
    let kill_signal = KillSignal::default();
    let _drop_signal = kill_signal.clone().drop_guard();

    let mut env_vars = crate::task_runner::real_env_vars();
    // so the subprocess can detect that it is running as part of a lifecycle script,
    // and avoid trying to set up node_modules again
    env_vars.insert(LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR.into(), "1".into());
    // we want to pass the current state of npm resolution down to the deno subprocess
    // (that may be running as part of the script). we do this with an inherited temp file
    //
    // SAFETY: we are sharing a single temp file across all of the scripts. the file position
    // will be shared among these, which is okay since we run only one script at a time.
    // However, if we concurrently run scripts in the future we will
    // have to have multiple temp files.
    let temp_file_fd = deno_runtime::deno_process::npm_process_state_tempfile(
      options.process_state.as_bytes(),
    )
    .map_err(DenoTaskLifecycleScriptsError::CreateNpmProcessState)?;
    // SAFETY: fd/handle is valid
    let _temp_file = unsafe { std::fs::File::from_raw_io_handle(temp_file_fd) }; // make sure the file gets closed
    env_vars.insert(
      deno_runtime::deno_process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME.into(),
      (temp_file_fd as usize).to_string().into(),
    );
    for PackageWithScript {
      package,
      scripts,
      package_folder,
    } in options.packages_with_scripts
    {
      // add custom commands for binaries from the package's dependencies. this will take precedence over the
      // baseline commands, so if the package relies on a bin that conflicts with one higher in the dependency tree, the
      // correct bin will be used.
      let custom_commands = self
        .resolve_custom_commands_from_deps(
          options.extra_info_provider,
          base.clone(),
          package,
          options.snapshot,
        )
        .await;
      for script_name in ["preinstall", "install", "postinstall"] {
        if let Some(script) = scripts.get(script_name) {
          if script_name == "install"
            && is_broken_default_install_script(script, package_folder)
          {
            continue;
          }
          let _guard = options.progress_bar.update_with_prompt(
            ProgressMessagePrompt::Initialize,
            &format!("{}: running '{script_name}' script", package.id.nv),
          );
          let crate::task_runner::TaskResult {
            exit_code,
            stderr,
            stdout,
          } =
            crate::task_runner::run_task(crate::task_runner::RunTaskOptions {
              task_name: script_name,
              script,
              cwd: package_folder.clone(),
              env_vars: env_vars.clone(),
              custom_commands: custom_commands.clone(),
              init_cwd: options.init_cwd,
              argv: &[],
              root_node_modules_dir: Some(options.root_node_modules_dir_path),
              stdio: Some(crate::task_runner::TaskIo {
                stderr: TaskStdio::piped(),
                stdout: TaskStdio::piped(),
              }),
              kill_signal: kill_signal.clone(),
            })
            .await
            .map_err(DenoTaskLifecycleScriptsError::Task)?;
          let stdout = stdout.unwrap();
          let stderr = stderr.unwrap();
          if exit_code != 0 {
            log::warn!(
              "error: script '{}' in '{}' failed with exit code {}{}{}",
              script_name,
              package.id.nv,
              exit_code,
              if !stdout.trim_ascii().is_empty() {
                format!(
                  "\nstdout:\n{}\n",
                  String::from_utf8_lossy(&stdout).trim()
                )
              } else {
                String::new()
              },
              if !stderr.trim_ascii().is_empty() {
                format!(
                  "\nstderr:\n{}\n",
                  String::from_utf8_lossy(&stderr).trim()
                )
              } else {
                String::new()
              },
            );
            failed_packages.push(&package.id.nv);
            // assume if earlier script fails, later ones will fail too
            break;
          }
        }
      }
      (options.on_ran_pkg_scripts)(package)?;
    }

    // re-set up bin entries for the packages which we've run scripts for.
    // lifecycle scripts can create files that are linked to by bin entries,
    // and the only reliable way to handle this is to re-link bin entries
    // (this is what PNPM does as well)
    let package_ids = options
      .packages_with_scripts
      .iter()
      .map(|p| &p.package.id)
      .collect::<HashSet<_>>();
    bin_entries.finish_only(
      options.snapshot,
      &options.root_node_modules_dir_path.join(".bin"),
      |outcome| outcome.warn_if_failed(),
      &package_ids,
    )?;

    if failed_packages.is_empty() {
      Ok(())
    } else {
      Err(
        DenoTaskLifecycleScriptsError::RunScripts(
          failed_packages
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>(),
        )
        .into(),
      )
    }
  }
}

impl DenoTaskLifeCycleScriptsExecutor {
  pub fn new(npm_resolver: ManagedNpmResolverRc<CliSys>) -> Self {
    Self { npm_resolver }
  }

  // take in all (non copy) packages from snapshot,
  // and resolve the set of available binaries to create
  // custom commands available to the task runner
  async fn resolve_baseline_custom_commands<'a>(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    bin_entries: &mut BinEntries<'a>,
    snapshot: &'a NpmResolutionSnapshot,
    packages: &'a [NpmResolutionPackage],
  ) -> crate::task_runner::TaskCustomCommands {
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
    self
      .resolve_custom_commands_from_packages(
        extra_info_provider,
        bin_entries,
        custom_commands,
        snapshot,
        packages,
      )
      .await
  }

  // resolves the custom commands from an iterator of packages
  // and adds them to the existing custom commands.
  // note that this will overwrite any existing custom commands
  async fn resolve_custom_commands_from_packages<
    'a,
    P: IntoIterator<Item = &'a NpmResolutionPackage>,
  >(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    bin_entries: &mut BinEntries<'a>,
    mut commands: crate::task_runner::TaskCustomCommands,
    snapshot: &'a NpmResolutionSnapshot,
    packages: P,
  ) -> crate::task_runner::TaskCustomCommands {
    for package in packages {
      let Ok(package_path) = self
        .npm_resolver
        .resolve_pkg_folder_from_pkg_id(&package.id)
      else {
        continue;
      };
      let extra = if let Some(extra) = &package.extra {
        Cow::Borrowed(extra)
      } else {
        let Ok(extra) = extra_info_provider
          .get_package_extra_info(
            &package.id.nv,
            &package_path,
            ExpectedExtraInfo::from_package(package),
          )
          .await
        else {
          continue;
        };
        Cow::Owned(extra)
      };
      if extra.bin.is_some() {
        bin_entries.add(package, &extra, package_path);
      }
    }

    let bins: Vec<(String, PathBuf)> = bin_entries.collect_bin_files(snapshot);
    for (bin_name, script_path) in bins {
      commands.insert(
        bin_name.clone(),
        Rc::new(crate::task_runner::NodeModulesFileRunCommand {
          command_name: bin_name,
          path: script_path,
        }),
      );
    }

    commands
  }

  // resolves the custom commands from the dependencies of a package
  // and adds them to the existing custom commands.
  // note that this will overwrite any existing custom commands.
  async fn resolve_custom_commands_from_deps(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    baseline: crate::task_runner::TaskCustomCommands,
    package: &NpmResolutionPackage,
    snapshot: &NpmResolutionSnapshot,
  ) -> crate::task_runner::TaskCustomCommands {
    let mut bin_entries = BinEntries::new();
    self
      .resolve_custom_commands_from_packages(
        extra_info_provider,
        &mut bin_entries,
        baseline,
        snapshot,
        package
          .dependencies
          .values()
          .map(|id| snapshot.package_from_id(id).unwrap()),
      )
      .await
  }
}
