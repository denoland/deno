// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_config::workspace::Workspace;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;
use deno_lockfile::WorkspaceMemberConfig;
use deno_package_json::PackageJsonDepValue;
use deno_runtime::deno_node::PackageJson;
use deno_semver::jsr::JsrDepPackageReq;

use crate::cache;
use crate::util::fs::atomic_write_file_with_retries;
use crate::Flags;

use crate::args::DenoSubcommand;
use crate::args::InstallFlags;
use crate::args::InstallKind;

use deno_lockfile::Lockfile;

#[derive(Debug)]
pub struct CliLockfileReadFromPathOptions {
  pub file_path: PathBuf,
  pub frozen: bool,
  /// Causes the lockfile to only be read from, but not written to.
  pub skip_write: bool,
}

#[derive(Debug)]
pub struct CliLockfile {
  lockfile: Mutex<Lockfile>,
  pub filename: PathBuf,
  frozen: bool,
  skip_write: bool,
}

pub struct Guard<'a, T> {
  guard: MutexGuard<'a, T>,
}

impl<'a, T> std::ops::Deref for Guard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<'a, T> std::ops::DerefMut for Guard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

impl CliLockfile {
  /// Get the inner deno_lockfile::Lockfile.
  pub fn lock(&self) -> Guard<Lockfile> {
    Guard {
      guard: self.lockfile.lock(),
    }
  }

  pub fn set_workspace_config(
    &self,
    options: deno_lockfile::SetWorkspaceConfigOptions,
  ) {
    self.lockfile.lock().set_workspace_config(options);
  }

  pub fn overwrite(&self) -> bool {
    self.lockfile.lock().overwrite
  }

  pub fn write_if_changed(&self) -> Result<(), AnyError> {
    if self.skip_write {
      return Ok(());
    }

    self.error_if_changed()?;
    let mut lockfile = self.lockfile.lock();
    let Some(bytes) = lockfile.resolve_write_bytes() else {
      return Ok(()); // nothing to do
    };
    // do an atomic write to reduce the chance of multiple deno
    // processes corrupting the file
    atomic_write_file_with_retries(
      &lockfile.filename,
      bytes,
      cache::CACHE_PERM,
    )
    .context("Failed writing lockfile.")?;
    lockfile.has_content_changed = false;
    Ok(())
  }

  pub fn discover(
    flags: &Flags,
    workspace: &Workspace,
  ) -> Result<Option<CliLockfile>, AnyError> {
    fn pkg_json_deps(
      maybe_pkg_json: Option<&PackageJson>,
    ) -> HashSet<JsrDepPackageReq> {
      let Some(pkg_json) = maybe_pkg_json else {
        return Default::default();
      };
      pkg_json
        .resolve_local_package_json_deps()
        .values()
        .filter_map(|dep| dep.as_ref().ok())
        .filter_map(|dep| match dep {
          PackageJsonDepValue::Req(req) => {
            Some(JsrDepPackageReq::npm(req.clone()))
          }
          PackageJsonDepValue::Workspace(_) => None,
        })
        .collect()
    }

    fn deno_json_deps(
      maybe_deno_json: Option<&ConfigFile>,
    ) -> HashSet<JsrDepPackageReq> {
      maybe_deno_json
        .map(|c| {
          crate::args::deno_json::deno_json_deps(c)
            .into_iter()
            .collect()
        })
        .unwrap_or_default()
    }

    if flags.no_lock
      || matches!(
        flags.subcommand,
        DenoSubcommand::Install(InstallFlags {
          kind: InstallKind::Global(..),
          ..
        }) | DenoSubcommand::Uninstall(_)
      )
    {
      return Ok(None);
    }

    let file_path = match flags.lock {
      Some(ref lock) => PathBuf::from(lock),
      None => match workspace.resolve_lockfile_path()? {
        Some(path) => path,
        None => return Ok(None),
      },
    };

    let root_folder = workspace.root_folder_configs();
    // CLI flag takes precedence over the config
    let frozen = flags.frozen_lockfile.unwrap_or_else(|| {
      root_folder
        .deno_json
        .as_ref()
        .and_then(|c| c.to_lock_config().ok().flatten().map(|c| c.frozen()))
        .unwrap_or(false)
    });

    let lockfile = Self::read_from_path(CliLockfileReadFromPathOptions {
      file_path,
      frozen,
      skip_write: flags.internal.lockfile_skip_write,
    })?;

    // initialize the lockfile with the workspace's configuration
    let root_url = workspace.root_dir();
    let config = deno_lockfile::WorkspaceConfig {
      root: WorkspaceMemberConfig {
        package_json_deps: pkg_json_deps(root_folder.pkg_json.as_deref()),
        dependencies: deno_json_deps(root_folder.deno_json.as_deref()),
      },
      members: workspace
        .config_folders()
        .iter()
        .filter(|(folder_url, _)| *folder_url != root_url)
        .filter_map(|(folder_url, folder)| {
          Some((
            {
              // should never be None here, but just ignore members that
              // do fail for this
              let mut relative_path = root_url.make_relative(folder_url)?;
              if relative_path.ends_with('/') {
                // make it slightly cleaner by removing the trailing slash
                relative_path.pop();
              }
              relative_path
            },
            {
              let config = WorkspaceMemberConfig {
                package_json_deps: pkg_json_deps(folder.pkg_json.as_deref()),
                dependencies: deno_json_deps(folder.deno_json.as_deref()),
              };
              if config.package_json_deps.is_empty()
                && config.dependencies.is_empty()
              {
                // exclude empty workspace members
                return None;
              }
              config
            },
          ))
        })
        .collect(),
    };
    lockfile.set_workspace_config(deno_lockfile::SetWorkspaceConfigOptions {
      no_npm: flags.no_npm,
      no_config: flags.config_flag == super::ConfigFlag::Disabled,
      config,
    });

    Ok(Some(lockfile))
  }

  pub fn read_from_path(
    opts: CliLockfileReadFromPathOptions,
  ) -> Result<CliLockfile, AnyError> {
    let lockfile = match std::fs::read_to_string(&opts.file_path) {
      Ok(text) => Lockfile::new(deno_lockfile::NewLockfileOptions {
        file_path: opts.file_path,
        content: &text,
        overwrite: false,
      })?,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        Lockfile::new_empty(opts.file_path, false)
      }
      Err(err) => {
        return Err(err).with_context(|| {
          format!("Failed reading lockfile '{}'", opts.file_path.display())
        });
      }
    };
    Ok(CliLockfile {
      filename: lockfile.filename.clone(),
      lockfile: Mutex::new(lockfile),
      frozen: opts.frozen,
      skip_write: opts.skip_write,
    })
  }

  pub fn error_if_changed(&self) -> Result<(), AnyError> {
    if !self.frozen {
      return Ok(());
    }
    let lockfile = self.lockfile.lock();
    if lockfile.has_content_changed {
      let contents =
        std::fs::read_to_string(&lockfile.filename).unwrap_or_default();
      let new_contents = lockfile.as_json_string();
      let diff = crate::util::diff::diff(&contents, &new_contents);
      // has an extra newline at the end
      let diff = diff.trim_end();
      Err(deno_core::anyhow::anyhow!(
        "The lockfile is out of date. Run `deno install --frozen=false`, or rerun with `--frozen=false` to update it.\nchanges:\n{diff}"
      ))
    } else {
      Ok(())
    }
  }
}
