// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod common;
mod global;
mod local;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_graph::npm::NpmPackageReq;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::PathClean;
use deno_runtime::deno_node::RequireNpmResolver;
use global::GlobalNpmPackageResolver;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::args::Lockfile;
use crate::util::fs::canonicalize_path_maybe_not_exists;

use self::common::InnerNpmPackageResolver;
use self::local::LocalNpmPackageResolver;
use super::NpmCache;
use super::NpmPackageNodeId;
use super::NpmResolutionSnapshot;
use super::RealNpmRegistryApi;

/// State provided to the process via an environment variable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub snapshot: NpmResolutionSnapshot,
  pub local_node_modules_path: Option<String>,
}

#[derive(Clone)]
pub struct NpmPackageResolver {
  no_npm: bool,
  inner: Arc<dyn InnerNpmPackageResolver>,
  local_node_modules_path: Option<PathBuf>,
  api: RealNpmRegistryApi,
  cache: NpmCache,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
}

impl std::fmt::Debug for NpmPackageResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NpmPackageResolver")
      .field("no_npm", &self.no_npm)
      .field("inner", &"<omitted>")
      .field("local_node_modules_path", &self.local_node_modules_path)
      .finish()
  }
}

impl NpmPackageResolver {
  pub fn new(cache: NpmCache, api: RealNpmRegistryApi) -> Self {
    Self::new_inner(cache, api, false, None, None, None)
  }

  pub async fn new_with_maybe_lockfile(
    cache: NpmCache,
    api: RealNpmRegistryApi,
    no_npm: bool,
    local_node_modules_path: Option<PathBuf>,
    initial_snapshot: Option<NpmResolutionSnapshot>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Result<Self, AnyError> {
    let mut initial_snapshot = initial_snapshot;

    if initial_snapshot.is_none() {
      if let Some(lockfile) = &maybe_lockfile {
        if !lockfile.lock().overwrite {
          initial_snapshot = Some(
            NpmResolutionSnapshot::from_lockfile(lockfile.clone(), &api)
              .await
              .with_context(|| {
                format!(
                  "failed reading lockfile '{}'",
                  lockfile.lock().filename.display()
                )
              })?,
          )
        }
      }
    }

    Ok(Self::new_inner(
      cache,
      api,
      no_npm,
      local_node_modules_path,
      initial_snapshot,
      maybe_lockfile,
    ))
  }

  fn new_inner(
    cache: NpmCache,
    api: RealNpmRegistryApi,
    no_npm: bool,
    local_node_modules_path: Option<PathBuf>,
    maybe_snapshot: Option<NpmResolutionSnapshot>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let inner: Arc<dyn InnerNpmPackageResolver> = match &local_node_modules_path
    {
      Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
        cache.clone(),
        api.clone(),
        node_modules_folder.clone(),
        maybe_snapshot,
      )),
      None => Arc::new(GlobalNpmPackageResolver::new(
        cache.clone(),
        api.clone(),
        maybe_snapshot,
      )),
    };
    Self {
      no_npm,
      inner,
      local_node_modules_path,
      api,
      cache,
      maybe_lockfile,
    }
  }

  /// Resolves an npm package folder path from a Deno module.
  pub fn resolve_package_folder_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<PathBuf, AnyError> {
    let path = self
      .inner
      .resolve_package_folder_from_deno_module(pkg_req)?;
    let path = canonicalize_path_maybe_not_exists(&path)?;
    log::debug!(
      "Resolved package folder of {} to {}",
      pkg_req,
      path.display()
    );
    Ok(path)
  }

  /// Resolves an npm package folder path from an npm package referrer.
  pub fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError> {
    let path = self
      .inner
      .resolve_package_folder_from_package(name, referrer, mode)?;
    log::debug!("Resolved {} from {} to {}", name, referrer, path.display());
    Ok(path)
  }

  /// Resolve the root folder of the package the provided specifier is in.
  ///
  /// This will error when the provided specifier is not in an npm package.
  pub fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let path = self
      .inner
      .resolve_package_folder_from_specifier(specifier)?;
    log::debug!(
      "Resolved package folder of {} to {}",
      specifier,
      path.display()
    );
    Ok(path)
  }

  /// Attempts to get the package size in bytes.
  pub fn package_size(
    &self,
    package_id: &NpmPackageNodeId,
  ) -> Result<u64, AnyError> {
    self.inner.package_size(package_id)
  }

  /// Gets if the provided specifier is in an npm package.
  pub fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .resolve_package_folder_from_specifier(specifier)
      .is_ok()
  }

  /// If the resolver has resolved any npm packages.
  pub fn has_packages(&self) -> bool {
    self.inner.has_packages()
  }

  /// Adds package requirements to the resolver and ensures everything is setup.
  pub async fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    if packages.is_empty() {
      return Ok(());
    }

    if self.no_npm {
      let fmt_reqs = packages
        .iter()
        .collect::<HashSet<_>>() // prevent duplicates
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
      return Err(custom_error(
        "NoNpm",
        format!(
          "Following npm specifiers were requested: {fmt_reqs}; but --no-npm is specified."
        ),
      ));
    }

    self.inner.add_package_reqs(packages).await?;
    self.inner.cache_packages().await?;

    // If there's a lock file, update it with all discovered npm packages
    if let Some(lockfile_mutex) = &self.maybe_lockfile {
      let mut lockfile = lockfile_mutex.lock();
      self.lock(&mut lockfile)?;
    }

    Ok(())
  }

  /// Sets package requirements to the resolver, removing old requirements and adding new ones.
  ///
  /// This will retrieve and resolve package information, but not cache any package files.
  pub async fn set_package_reqs(
    &self,
    packages: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    self.inner.set_package_reqs(packages).await
  }

  /// Gets the state of npm for the process.
  pub fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      snapshot: self.inner.snapshot(),
      local_node_modules_path: self
        .local_node_modules_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string()),
    })
    .unwrap()
  }

  /// Gets a new resolver with a new snapshotted state.
  pub fn snapshotted(&self) -> Self {
    Self::new_inner(
      self.cache.clone(),
      self.api.clone(),
      self.no_npm,
      self.local_node_modules_path.clone(),
      Some(self.snapshot()),
      None,
    )
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.inner.snapshot()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    self.inner.lock(lockfile)
  }

  pub async fn inject_synthetic_types_node_package(
    &self,
  ) -> Result<(), AnyError> {
    // add and ensure this isn't added to the lockfile
    self
      .inner
      .add_package_reqs(vec![NpmPackageReq::from_str("@types/node").unwrap()])
      .await?;
    self.inner.cache_packages().await?;

    Ok(())
  }
}

impl RequireNpmResolver for NpmPackageResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &std::path::Path,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError> {
    let referrer = path_to_specifier(referrer)?;
    self.resolve_package_folder_from_package(specifier, &referrer, mode)
  }

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError> {
    let specifier = path_to_specifier(path)?;
    self.resolve_package_folder_from_specifier(&specifier)
  }

  fn in_npm_package(&self, path: &Path) -> bool {
    let specifier =
      match ModuleSpecifier::from_file_path(path.to_path_buf().clean()) {
        Ok(p) => p,
        Err(_) => return false,
      };
    self
      .resolve_package_folder_from_specifier(&specifier)
      .is_ok()
  }

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    self.inner.ensure_read_permission(permissions, path)
  }
}

fn path_to_specifier(path: &Path) -> Result<ModuleSpecifier, AnyError> {
  match ModuleSpecifier::from_file_path(path.to_path_buf().clean()) {
    Ok(specifier) => Ok(specifier),
    Err(()) => bail!("Could not convert '{}' to url.", path.display()),
  }
}
