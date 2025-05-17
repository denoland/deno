// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Error as AnyError;
use deno_config::workspace::Workspace;
use deno_error::JsErrorBox;
use deno_lockfile::Lockfile;
use deno_lockfile::NpmPackageInfoProvider;
use deno_lockfile::WorkspaceMemberConfig;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::DefaultTarballUrlProvider;
use deno_npm::resolution::NpmRegistryDefaultTarballUrlProvider;
use deno_package_json::PackageJsonDepValue;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use futures::stream::FuturesOrdered;
use futures::TryStreamExt;
use indexmap::IndexMap;
use node_resolver::PackageJson;
use parking_lot::Mutex;
use parking_lot::MutexGuard;

use crate::sync::MaybeSend;
use crate::sync::MaybeSync;
use crate::workspace::WorkspaceNpmPatchPackagesRc;

pub trait NpmRegistryApiEx: NpmRegistryApi + MaybeSend + MaybeSync {}

impl<T> NpmRegistryApiEx for T where T: NpmRegistryApi + MaybeSend + MaybeSync {}

#[allow(clippy::disallowed_types)]
type NpmRegistryApiRc = crate::sync::MaybeArc<dyn NpmRegistryApiEx>;

pub struct LockfileNpmPackageInfoApiAdapter {
  api: NpmRegistryApiRc,
  workspace_patch_packages: WorkspaceNpmPatchPackagesRc,
}

impl LockfileNpmPackageInfoApiAdapter {
  pub fn new(
    api: NpmRegistryApiRc,
    workspace_patch_packages: WorkspaceNpmPatchPackagesRc,
  ) -> Self {
    Self {
      api,
      workspace_patch_packages,
    }
  }

  async fn get_infos(
    &self,
    values: &[PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    let futs = values
      .iter()
      .map(|v| async move {
        let info = self.api.package_info(v.name.as_str()).await?;
        let version_info =
          info.version_info(v, &self.workspace_patch_packages.0)?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
          deno_lockfile::Lockfile5NpmInfo {
            tarball_url: version_info.dist.as_ref().and_then(|d| {
              let tarball_url_provider = NpmRegistryDefaultTarballUrlProvider;
              if d.tarball == tarball_url_provider.default_tarball_url(v) {
                None
              } else {
                Some(d.tarball.clone())
              }
            }),
            optional_dependencies: version_info
              .optional_dependencies
              .iter()
              .map(|(k, v)| (k.to_string(), v.to_string()))
              .collect::<std::collections::BTreeMap<_, _>>(),
            cpu: version_info.cpu.iter().map(|s| s.to_string()).collect(),
            os: version_info.os.iter().map(|s| s.to_string()).collect(),
            deprecated: version_info.deprecated.is_some(),
            bin: version_info.bin.is_some(),
            scripts: version_info.scripts.contains_key("preinstall")
              || version_info.scripts.contains_key("install")
              || version_info.scripts.contains_key("postinstall"),
            optional_peers: version_info
              .peer_dependencies_meta
              .iter()
              .filter_map(|(k, v)| {
                if v.optional {
                  version_info
                    .peer_dependencies
                    .get(k)
                    .map(|v| (k.to_string(), v.to_string()))
                } else {
                  None
                }
              })
              .collect::<std::collections::BTreeMap<_, _>>(),
          },
        )
      })
      .collect::<FuturesOrdered<_>>();
    let package_infos = futs.try_collect::<Vec<_>>().await?;
    Ok(package_infos)
  }
}

#[async_trait::async_trait(?Send)]
impl deno_lockfile::NpmPackageInfoProvider
  for LockfileNpmPackageInfoApiAdapter
{
  async fn get_npm_package_info(
    &self,
    values: &[PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    let package_infos = self.get_infos(values).await;

    match package_infos {
      Ok(package_infos) => Ok(package_infos),
      Err(err) => {
        if self.api.mark_force_reload() {
          self.get_infos(values).await
        } else {
          Err(err)
        }
      }
    }
  }
}

#[derive(Debug)]
pub struct LockfileReadFromPathOptions {
  pub file_path: PathBuf,
  pub frozen: bool,
  /// Causes the lockfile to only be read from, but not written to.
  pub skip_write: bool,
}

#[sys_traits::auto_impl]
pub trait LockfileSys:
  deno_path_util::fs::AtomicWriteFileWithRetriesSys
  + sys_traits::FsRead
  + std::fmt::Debug
{
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

#[derive(Debug, Clone)]
pub struct LockfileFlags {
  pub no_lock: bool,
  pub frozen_lockfile: Option<bool>,
  pub lock: Option<PathBuf>,
  pub skip_write: bool,
  pub no_config: bool,
  pub no_npm: bool,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LockfileWriteError {
  #[class(inherit)]
  #[error(transparent)]
  Changed(JsErrorBox),
  #[class(inherit)]
  #[error("Failed writing lockfile")]
  Io(#[source] std::io::Error),
}

#[allow(clippy::disallowed_types)]
pub type LockfileLockRc<TSys> = crate::sync::MaybeArc<LockfileLock<TSys>>;

#[derive(Debug)]
pub struct LockfileLock<TSys: LockfileSys> {
  sys: TSys,
  lockfile: Mutex<Lockfile>,
  pub filename: PathBuf,
  frozen: bool,
  skip_write: bool,
}

impl<TSys: LockfileSys> LockfileLock<TSys> {
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

  pub fn write_if_changed(&self) -> Result<(), LockfileWriteError> {
    if self.skip_write {
      return Ok(());
    }

    self
      .error_if_changed()
      .map_err(LockfileWriteError::Changed)?;
    let mut lockfile = self.lockfile.lock();
    let Some(bytes) = lockfile.resolve_write_bytes() else {
      return Ok(()); // nothing to do
    };
    // do an atomic write to reduce the chance of multiple deno
    // processes corrupting the file
    const CACHE_PERM: u32 = 0o644;
    atomic_write_file_with_retries(
      &self.sys,
      &lockfile.filename,
      &bytes,
      CACHE_PERM,
    )
    .map_err(LockfileWriteError::Io)?;
    lockfile.has_content_changed = false;
    Ok(())
  }

  pub async fn discover(
    sys: TSys,
    flags: LockfileFlags,
    workspace: &Workspace,
    maybe_external_import_map: Option<&serde_json::Value>,
    api: &dyn NpmPackageInfoProvider,
  ) -> Result<Option<Self>, AnyError> {
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

    if flags.no_lock {
      return Ok(None);
    }
    let file_path = match flags.lock {
      Some(path) => path,
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
      LockfileReadFromPathOptions {
        file_path,
        frozen,
        skip_write: flags.skip_write,
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
          root_folder
            .deno_json
            .as_deref()
            .map(deno_json_deps)
            .unwrap_or_default()
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
                dependencies: folder
                  .deno_json
                  .as_deref()
                  .map(deno_json_deps)
                  .unwrap_or_default(),
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
      no_config: flags.no_config,
      config,
    });
    Ok(Some(lockfile))
  }

  pub async fn read_from_path(
    sys: TSys,
    opts: LockfileReadFromPathOptions,
    api: &dyn deno_lockfile::NpmPackageInfoProvider,
  ) -> Result<LockfileLock<TSys>, AnyError> {
    let lockfile = match sys.fs_read_to_string(&opts.file_path) {
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
    Ok(LockfileLock {
      sys,
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
      let contents = self
        .sys
        .fs_read_to_string(&lockfile.filename)
        .unwrap_or_default();
      let new_contents = lockfile.as_json_string();
      let diff = crate::display::diff(&contents, &new_contents);
      // has an extra newline at the end
      let diff = diff.trim_end();
      Err(JsErrorBox::generic(format!("The lockfile is out of date. Run `deno install --frozen=false`, or rerun with `--frozen=false` to update it.\nchanges:\n{diff}")))
    } else {
      Ok(())
    }
  }
}

fn import_map_deps(
  import_map: &serde_json::Value,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(import_map.get("imports"))
    .into_iter()
    .chain(scope_values(import_map.get("scopes")));
  values_to_set(values)
}

fn deno_json_deps(
  config: &deno_config::deno_json::ConfigFile,
) -> HashSet<JsrDepPackageReq> {
  let values = imports_values(config.json.imports.as_ref())
    .into_iter()
    .chain(scope_values(config.json.scopes.as_ref()));
  let mut set = values_to_set(values);

  if let Some(serde_json::Value::Object(compiler_options)) =
    &config.json.compiler_options
  {
    // add jsxImportSource
    if let Some(serde_json::Value::String(value)) =
      compiler_options.get("jsxImportSource")
    {
      if let Some(dep_req) = value_to_dep_req(value) {
        set.insert(dep_req);
      }
    }
    // add jsxImportSourceTypes
    if let Some(serde_json::Value::String(value)) =
      compiler_options.get("jsxImportSourceTypes")
    {
      if let Some(dep_req) = value_to_dep_req(value) {
        set.insert(dep_req);
      }
    }
    // add the dependencies in the types array
    if let Some(serde_json::Value::Array(types)) = compiler_options.get("types")
    {
      for value in types {
        if let serde_json::Value::String(value) = value {
          if let Some(dep_req) = value_to_dep_req(value) {
            set.insert(dep_req);
          }
        }
      }
    }
  }

  set
}

fn imports_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  let mut items = Vec::with_capacity(obj.len());
  for value in obj.values() {
    if let serde_json::Value::String(value) = value {
      items.push(value);
    }
  }
  items
}

fn scope_values(value: Option<&serde_json::Value>) -> Vec<&String> {
  let Some(obj) = value.and_then(|v| v.as_object()) else {
    return Vec::new();
  };
  obj.values().flat_map(|v| imports_values(Some(v))).collect()
}

fn values_to_set<'a>(
  values: impl Iterator<Item = &'a String>,
) -> HashSet<JsrDepPackageReq> {
  let mut entries = HashSet::new();
  for value in values {
    if let Some(dep_req) = value_to_dep_req(value) {
      entries.insert(dep_req);
    }
  }
  entries
}

fn value_to_dep_req(value: &str) -> Option<JsrDepPackageReq> {
  if let Ok(req_ref) = JsrPackageReqReference::from_str(value) {
    Some(JsrDepPackageReq::jsr(req_ref.into_inner().req))
  } else if let Ok(req_ref) = NpmPackageReqReference::from_str(value) {
    Some(JsrDepPackageReq::npm(req_ref.into_inner().req))
  } else {
    None
  }
}
