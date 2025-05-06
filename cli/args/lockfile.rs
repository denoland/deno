// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::PathBuf;

use deno_config::deno_json::ConfigFile;
use deno_config::workspace::Workspace;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_lockfile::Lockfile;
use deno_lockfile::NpmPackageInfoProvider;
use deno_lockfile::WorkspaceMemberConfig;
use deno_package_json::PackageJsonDepValue;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_runtime::deno_node::PackageJson;
use deno_semver::jsr::JsrDepPackageReq;
use indexmap::IndexMap;

use crate::args::deno_json::import_map_deps;
use crate::args::DenoSubcommand;
use crate::args::InstallFlags;
use crate::cache;
use crate::sys::CliSys;
use crate::Flags;

#[derive(Debug)]
pub struct CliLockfileReadFromPathOptions {
  pub file_path: PathBuf,
  pub frozen: bool,
  /// Causes the lockfile to only be read from, but not written to.
  pub skip_write: bool,
}

#[derive(Debug)]
pub struct CliLockfile {
  sys: CliSys,
  lockfile: Mutex<Lockfile>,
  pub filename: PathBuf,
  frozen: bool,
  skip_write: bool,
}

pub struct Guard<'a, T> {
  guard: MutexGuard<'a, T>,
}

impl<T> std::ops::Deref for Guard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> std::ops::DerefMut for Guard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum AtomicWriteFileWithRetriesError {
  #[class(inherit)]
  #[error(transparent)]
  Changed(JsErrorBox),
  #[class(inherit)]
  #[error("Failed writing lockfile")]
  Io(#[source] std::io::Error),
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

  pub fn write_if_changed(
    &self,
  ) -> Result<(), AtomicWriteFileWithRetriesError> {
    if self.skip_write {
      return Ok(());
    }

    self
      .error_if_changed()
      .map_err(AtomicWriteFileWithRetriesError::Changed)?;
    let mut lockfile = self.lockfile.lock();
    let Some(bytes) = lockfile.resolve_write_bytes() else {
      return Ok(()); // nothing to do
    };
    // do an atomic write to reduce the chance of multiple deno
    // processes corrupting the file
    atomic_write_file_with_retries(
      &self.sys,
      &lockfile.filename,
      &bytes,
      cache::CACHE_PERM,
    )
    .map_err(AtomicWriteFileWithRetriesError::Io)?;
    lockfile.has_content_changed = false;
    Ok(())
  }

  pub async fn discover(
    sys: &CliSys,
    flags: &Flags,
    workspace: &Workspace,
    maybe_external_import_map: Option<&serde_json::Value>,
    api: &(dyn NpmPackageInfoProvider + Send + Sync),
  ) -> Result<Option<CliLockfile>, AnyError> {
    fn pkg_json_deps(
      maybe_pkg_json: Option<&PackageJson>,
    ) -> HashSet<JsrDepPackageReq> {
      let Some(pkg_json) = maybe_pkg_json else {
        return Default::default();
      };
      let deps = pkg_json.resolve_local_package_json_deps();

      deps
        .dependencies
        .values()
        .chain(deps.dev_dependencies.values())
        .filter_map(|dep| dep.as_ref().ok())
        .filter_map(|dep| match dep {
          PackageJsonDepValue::File(_) => {
            // ignored because this will have its own separate lockfile
            None
          }
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
        .map(crate::args::deno_json::deno_json_deps)
        .unwrap_or_default()
    }
    if flags.no_lock
      || matches!(
        flags.subcommand,
        DenoSubcommand::Install(InstallFlags::Global(..))
          | DenoSubcommand::Uninstall(_)
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
    let frozen = flags.frozen_lockfile.unwrap_or_else(|| {
      root_folder
        .deno_json
        .as_ref()
        .and_then(|c| c.to_lock_config().ok().flatten().map(|c| c.frozen()))
        .unwrap_or(false)
    });
    let lockfile = Self::read_from_path(
      sys,
      CliLockfileReadFromPathOptions {
        file_path,
        frozen,
        skip_write: flags.internal.lockfile_skip_write,
      },
      api,
    )
    .await?;
    let root_url = workspace.root_dir();
    let config = deno_lockfile::WorkspaceConfig {
      root: WorkspaceMemberConfig {
        package_json_deps: pkg_json_deps(root_folder.pkg_json.as_deref()),
        dependencies: if let Some(map) = maybe_external_import_map {
          import_map_deps(map)
        } else {
          deno_json_deps(root_folder.deno_json.as_deref())
        },
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
      patches: if workspace.has_unstable("npm-patch") {
        workspace
          .patch_pkg_jsons()
          .filter_map(|pkg_json| {
            fn collect_deps(
              deps: Option<&IndexMap<String, String>>,
            ) -> HashSet<JsrDepPackageReq> {
              deps
                .map(|i| {
                  i.iter()
                    .filter_map(|(k, v)| PackageJsonDepValue::parse(k, v).ok())
                    .filter_map(|dep| match dep {
                      PackageJsonDepValue::Req(req) => {
                        Some(JsrDepPackageReq::npm(req.clone()))
                      }
                      // not supported
                      PackageJsonDepValue::File(_)
                      | PackageJsonDepValue::Workspace(_) => None,
                    })
                    .collect()
                })
                .unwrap_or_default()
            }

            let key = format!(
              "npm:{}@{}",
              pkg_json.name.as_ref()?,
              pkg_json.version.as_ref()?
            );
            // anything that affects npm resolution should go here in order to bust
            // the npm resolution when it changes
            let value = deno_lockfile::LockfilePatchContent {
              dependencies: collect_deps(pkg_json.dependencies.as_ref()),
              peer_dependencies: collect_deps(
                pkg_json.peer_dependencies.as_ref(),
              ),
              peer_dependencies_meta: pkg_json
                .peer_dependencies_meta
                .clone()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            };
            Some((key, value))
          })
          .collect()
      } else {
        Default::default()
      },
    };
    lockfile.set_workspace_config(deno_lockfile::SetWorkspaceConfigOptions {
      no_npm: flags.no_npm,
      no_config: flags.config_flag == super::ConfigFlag::Disabled,
      config,
    });
    Ok(Some(lockfile))
  }

  pub async fn read_from_path(
    sys: &CliSys,
    opts: CliLockfileReadFromPathOptions,
    api: &(dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync),
  ) -> Result<CliLockfile, AnyError> {
    let lockfile = match std::fs::read_to_string(&opts.file_path) {
      Ok(text) => {
        Lockfile::new(
          deno_lockfile::NewLockfileOptions {
            file_path: opts.file_path,
            content: &text,
            overwrite: false,
          },
          api,
        )
        .await?
      }
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
      sys: sys.clone(),
      filename: lockfile.filename.clone(),
      lockfile: Mutex::new(lockfile),
      frozen: opts.frozen,
      skip_write: opts.skip_write,
    })
  }

  pub fn error_if_changed(&self) -> Result<(), JsErrorBox> {
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
      Err(JsErrorBox::generic(format!("The lockfile is out of date. Run `deno install --frozen=false`, or rerun with `--frozen=false` to update it.\nchanges:\n{diff}")))
    } else {
      Ok(())
    }
  }
}
