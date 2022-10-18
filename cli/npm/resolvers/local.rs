// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! Code for local node_modules resolution.

use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_runtime::deno_core::futures;
use tokio::task::JoinHandle;

use crate::fs_util;
use crate::npm::cache::should_sync_download;
use crate::npm::resolution::NpmResolution;
use crate::npm::resolution::NpmResolutionSnapshot;
use crate::npm::NpmCache;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmRegistryApi;

use super::common::ensure_registry_read_permission;
use super::common::InnerNpmPackageResolver;

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
#[derive(Debug, Clone)]
pub struct LocalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  registry_url: Url,
  root_node_modules_path: PathBuf,
  root_node_modules_specifier: ModuleSpecifier,
}

impl LocalNpmPackageResolver {
  pub fn new(
    cache: NpmCache,
    api: NpmRegistryApi,
    node_modules_folder: PathBuf,
    initial_snapshot: Option<NpmResolutionSnapshot>,
  ) -> Self {
    let registry_url = api.base_url().to_owned();
    let resolution = Arc::new(NpmResolution::new(api, initial_snapshot));

    Self {
      cache,
      resolution,
      registry_url,
      root_node_modules_specifier: ModuleSpecifier::from_directory_path(
        &node_modules_folder,
      )
      .unwrap(),
      root_node_modules_path: node_modules_folder,
    }
  }

  fn resolve_package_root(&self, path: &Path) -> PathBuf {
    let mut last_found = path;
    loop {
      let parent = last_found.parent().unwrap();
      if parent.file_name().unwrap() == "node_modules" {
        return last_found.to_path_buf();
      } else {
        last_found = parent;
      }
    }
  }

  fn resolve_folder_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    match self.maybe_resolve_folder_for_specifier(specifier) {
      Some(path) => Ok(path),
      None => bail!("could not find npm package for '{}'", specifier),
    }
  }

  fn maybe_resolve_folder_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<PathBuf> {
    let relative_url =
      self.root_node_modules_specifier.make_relative(specifier)?;
    if relative_url.starts_with("../") {
      return None;
    }
    // it's within the directory, so use it
    specifier.to_file_path().ok()
  }
}

impl InnerNpmPackageResolver for LocalNpmPackageResolver {
  fn resolve_package_folder_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<PathBuf, AnyError> {
    let resolved_package =
      self.resolution.resolve_package_from_deno_module(pkg_req)?;

    // it might be at the full path if there are duplicate names
    let fully_resolved_folder_path = join_package_name(
      &self.root_node_modules_path,
      &resolved_package.id.to_string(),
    );
    Ok(if fully_resolved_folder_path.exists() {
      fully_resolved_folder_path
    } else {
      join_package_name(&self.root_node_modules_path, &resolved_package.id.name)
    })
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let local_path = self.resolve_folder_for_specifier(referrer)?;
    let package_root_path = self.resolve_package_root(&local_path);
    let mut current_folder = package_root_path.as_path();
    loop {
      current_folder = get_next_node_modules_ancestor(current_folder);
      let sub_dir = join_package_name(current_folder, name);
      if sub_dir.is_dir() {
        return Ok(sub_dir);
      }
      if current_folder == self.root_node_modules_path {
        bail!(
          "could not find package '{}' from referrer '{}'.",
          name,
          referrer
        );
      }
    }
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let local_path = self.resolve_folder_for_specifier(specifier)?;
    let package_root_path = self.resolve_package_root(&local_path);
    Ok(package_root_path)
  }

  fn has_packages(&self) -> bool {
    self.resolution.has_packages()
  }

  fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> BoxFuture<'static, Result<(), AnyError>> {
    let resolver = self.clone();
    async move {
      resolver.resolution.add_package_reqs(packages).await?;

      sync_resolution_with_fs(
        &resolver.resolution.snapshot(),
        &resolver.cache,
        &resolver.registry_url,
        &resolver.root_node_modules_path,
      )
      .await?;

      Ok(())
    }
    .boxed()
  }

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError> {
    ensure_registry_read_permission(&self.root_node_modules_path, path)
  }

  fn snapshot(&self) -> NpmResolutionSnapshot {
    self.resolution.snapshot()
  }
}

/// Creates a pnpm style folder structure.
async fn sync_resolution_with_fs(
  snapshot: &NpmResolutionSnapshot,
  cache: &NpmCache,
  registry_url: &Url,
  root_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  fn get_package_folder_name(package_id: &NpmPackageId) -> String {
    package_id.to_string().replace('/', "+")
  }

  let deno_local_registry_dir = root_node_modules_dir_path.join(".deno");
  fs::create_dir_all(&deno_local_registry_dir).with_context(|| {
    format!("Creating '{}'", deno_local_registry_dir.display())
  })?;

  // 1. Write all the packages out the .deno directory.
  //
  // Copy (hardlink in future) <global_registry_cache>/<package_id>/ to
  // node_modules/.deno/<package_id>/node_modules/<package_name>
  let sync_download = should_sync_download();
  let mut all_packages = snapshot.all_packages();
  if sync_download {
    // we're running the tests not with --quiet
    // and we want the output to be deterministic
    all_packages.sort_by(|a, b| a.id.cmp(&b.id));
  }
  let mut handles: Vec<JoinHandle<Result<(), AnyError>>> =
    Vec::with_capacity(all_packages.len());
  for package in &all_packages {
    let folder_name = get_package_folder_name(&package.id);
    let folder_path = deno_local_registry_dir.join(&folder_name);
    let initialized_file = folder_path.join("deno_initialized");
    if !initialized_file.exists() {
      let cache = cache.clone();
      let registry_url = registry_url.clone();
      let package = package.clone();
      let handle = tokio::task::spawn(async move {
        cache
          .ensure_package(&package.id, &package.dist, &registry_url)
          .await?;
        let sub_node_modules = folder_path.join("node_modules");
        let package_path =
          join_package_name(&sub_node_modules, &package.id.name);
        fs::create_dir_all(&package_path)
          .with_context(|| format!("Creating '{}'", folder_path.display()))?;
        let cache_folder = cache.package_folder(&package.id, &registry_url);
        // for now copy, but in the future consider hard linking
        fs_util::copy_dir_recursive(&cache_folder, &package_path)?;
        // write out a file that indicates this folder has been initialized
        fs::write(initialized_file, "")?;
        Ok(())
      });
      if sync_download {
        handle.await??;
      } else {
        handles.push(handle);
      }
    }
  }

  let results = futures::future::join_all(handles).await;
  for result in results {
    result??; // surface the first error
  }

  // 2. Symlink all the dependencies into the .deno directory.
  //
  // Symlink node_modules/.deno/<package_id>/node_modules/<dep_name> to
  // node_modules/.deno/<dep_id>/node_modules/<dep_package_name>
  for package in &all_packages {
    let sub_node_modules = deno_local_registry_dir
      .join(&get_package_folder_name(&package.id))
      .join("node_modules");
    for (name, dep_id) in &package.dependencies {
      let dep_folder_name = get_package_folder_name(dep_id);
      let dep_folder_path = join_package_name(
        &deno_local_registry_dir
          .join(dep_folder_name)
          .join("node_modules"),
        &dep_id.name,
      );
      symlink_package_dir(
        &dep_folder_path,
        &join_package_name(&sub_node_modules, name),
      )?;
    }
  }

  // 3. Create all the packages in the node_modules folder, which are symlinks.
  //
  // Symlink node_modules/<package_name> to
  // node_modules/.deno/<package_id>/node_modules/<package_name>
  let mut found_names = HashSet::new();
  let mut pending_packages = VecDeque::new();
  pending_packages.extend(
    snapshot
      .top_level_packages()
      .into_iter()
      .map(|id| (id, true)),
  );
  while let Some((package_id, is_top_level)) = pending_packages.pop_front() {
    let root_folder_name = if found_names.insert(package_id.name.clone()) {
      package_id.name.clone()
    } else if is_top_level {
      package_id.to_string()
    } else {
      continue; // skip, already handled
    };
    let local_registry_package_path = deno_local_registry_dir
      .join(&get_package_folder_name(&package_id))
      .join("node_modules")
      .join(&package_id.name);

    symlink_package_dir(
      &local_registry_package_path,
      &join_package_name(root_node_modules_dir_path, &root_folder_name),
    )?;
    if let Some(package) = snapshot.package_from_id(&package_id) {
      for id in package.dependencies.values() {
        pending_packages.push_back((id.clone(), false));
      }
    }
  }

  Ok(())
}

fn symlink_package_dir(
  old_path: &Path,
  new_path: &Path,
) -> Result<(), AnyError> {
  let new_parent = new_path.parent().unwrap();
  if new_parent.file_name().unwrap() != "node_modules" {
    // create the parent folder that will contain the symlink
    fs::create_dir_all(new_parent)
      .with_context(|| format!("Creating '{}'", new_parent.display()))?;
  }

  // need to delete the previous symlink before creating a new one
  let _ignore = fs::remove_dir_all(new_path);

  #[cfg(windows)]
  return junction_or_symlink_dir(old_path, new_path);
  #[cfg(not(windows))]
  fs_util::symlink_dir(old_path, new_path)
}

#[cfg(windows)]
fn junction_or_symlink_dir(
  old_path: &Path,
  new_path: &Path,
) -> Result<(), AnyError> {
  // Use junctions because they're supported on ntfs file systems without
  // needing to elevate privileges on Windows
  match junction::create(old_path, new_path) {
    Ok(()) => Ok(()),
    Err(junction_err) => {
      if cfg!(debug) {
        // When running the tests, junctions should be created, but if not then
        // surface this error.
        log::warn!("Error creating junction. {:#}", junction_err);
      }

      match fs_util::symlink_dir(old_path, new_path) {
        Ok(()) => Ok(()),
        Err(symlink_err) => bail!(
          concat!(
            "Failed creating junction and fallback symlink in node_modules folder.\n\n",
            "{:#}\n\n{:#}",
          ),
          junction_err,
          symlink_err,
        ),
      }
    }
  }
}

fn join_package_name(path: &Path, package_name: &str) -> PathBuf {
  let mut path = path.to_path_buf();
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = path.join(part);
  }
  path
}

fn get_next_node_modules_ancestor(mut path: &Path) -> &Path {
  loop {
    path = path.parent().unwrap();
    let file_name = path.file_name().unwrap().to_string_lossy();
    if file_name == "node_modules" {
      return path;
    }
  }
}
