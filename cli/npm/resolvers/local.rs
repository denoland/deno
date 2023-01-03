// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::util::fs::symlink_dir;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_runtime::deno_core::futures;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::PackageJson;
use tokio::task::JoinHandle;

use crate::args::Lockfile;
use crate::npm::cache::mixed_case_package_name_encode;
use crate::npm::cache::should_sync_download;
use crate::npm::cache::NpmPackageCacheFolderId;
use crate::npm::resolution::NpmResolution;
use crate::npm::resolution::NpmResolutionSnapshot;
use crate::npm::NpmCache;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmResolutionPackage;
use crate::npm::RealNpmRegistryApi;
use crate::util::fs::copy_dir_recursive;
use crate::util::fs::hard_link_dir_recursive;

use super::common::ensure_registry_read_permission;
use super::common::types_package_name;
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
    api: RealNpmRegistryApi,
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

  fn get_package_id_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, AnyError> {
    match self.resolution.resolve_package_from_id(package_id) {
      Some(package) => Ok(self.get_package_id_folder_from_package(&package)),
      None => bail!(
        "Could not find package information for '{}'",
        package_id.as_serialized()
      ),
    }
  }

  fn get_package_id_folder_from_package(
    &self,
    package: &NpmResolutionPackage,
  ) -> PathBuf {
    // package is stored at:
    // node_modules/.deno/<package_cache_folder_id_folder_name>/node_modules/<package_name>
    self
      .root_node_modules_path
      .join(".deno")
      .join(get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      ))
      .join("node_modules")
      .join(&package.id.name)
  }
}

impl InnerNpmPackageResolver for LocalNpmPackageResolver {
  fn resolve_package_folder_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<PathBuf, AnyError> {
    let package = self.resolution.resolve_package_from_deno_module(pkg_req)?;
    Ok(self.get_package_id_folder_from_package(&package))
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError> {
    let local_path = self.resolve_folder_for_specifier(referrer)?;
    let package_root_path = self.resolve_package_root(&local_path);
    let mut current_folder = package_root_path.as_path();
    loop {
      current_folder = get_next_node_modules_ancestor(current_folder);
      let sub_dir = join_package_name(current_folder, name);
      if sub_dir.is_dir() {
        // if doing types resolution, only resolve the package if it specifies a types property
        if mode.is_types() && !name.starts_with("@types/") {
          let package_json = PackageJson::load_skip_read_permission(
            sub_dir.join("package.json"),
          )?;
          if package_json.types.is_some() {
            return Ok(sub_dir);
          }
        } else {
          return Ok(sub_dir);
        }
      }

      // if doing type resolution, check for the existence of a @types package
      if mode.is_types() && !name.starts_with("@types/") {
        let sub_dir =
          join_package_name(current_folder, &types_package_name(name));
        if sub_dir.is_dir() {
          return Ok(sub_dir);
        }
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

  fn package_size(&self, package_id: &NpmPackageId) -> Result<u64, AnyError> {
    let package_folder_path = self.get_package_id_folder(package_id)?;

    Ok(crate::util::fs::dir_size(&package_folder_path)?)
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
      Ok(())
    }
    .boxed()
  }

  fn set_package_reqs(
    &self,
    packages: HashSet<NpmPackageReq>,
  ) -> BoxFuture<'static, Result<(), AnyError>> {
    let resolver = self.clone();
    async move {
      resolver.resolution.set_package_reqs(packages).await?;
      Ok(())
    }
    .boxed()
  }

  fn cache_packages(&self) -> BoxFuture<'static, Result<(), AnyError>> {
    let resolver = self.clone();
    async move {
      sync_resolver_with_fs(&resolver).await?;
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

  fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    self.resolution.lock(lockfile, &self.snapshot())
  }
}

async fn sync_resolver_with_fs(
  resolver: &LocalNpmPackageResolver,
) -> Result<(), AnyError> {
  sync_resolution_with_fs(
    &resolver.resolution.snapshot(),
    &resolver.cache,
    &resolver.registry_url,
    &resolver.root_node_modules_path,
  )
  .await
}

/// Creates a pnpm style folder structure.
async fn sync_resolution_with_fs(
  snapshot: &NpmResolutionSnapshot,
  cache: &NpmCache,
  registry_url: &Url,
  root_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  let deno_local_registry_dir = root_node_modules_dir_path.join(".deno");
  fs::create_dir_all(&deno_local_registry_dir).with_context(|| {
    format!("Creating '{}'", deno_local_registry_dir.display())
  })?;

  // 1. Write all the packages out the .deno directory.
  //
  // Copy (hardlink in future) <global_registry_cache>/<package_id>/ to
  // node_modules/.deno/<package_folder_id_folder_name>/node_modules/<package_name>
  let sync_download = should_sync_download();
  let mut package_partitions = snapshot.all_packages_partitioned();
  if sync_download {
    // we're running the tests not with --quiet
    // and we want the output to be deterministic
    package_partitions.packages.sort_by(|a, b| a.id.cmp(&b.id));
  }
  let mut handles: Vec<JoinHandle<Result<(), AnyError>>> =
    Vec::with_capacity(package_partitions.packages.len());
  for package in &package_partitions.packages {
    let folder_name =
      get_package_folder_id_folder_name(&package.get_package_cache_folder_id());
    let folder_path = deno_local_registry_dir.join(&folder_name);
    let initialized_file = folder_path.join(".initialized");
    if !cache
      .cache_setting()
      .should_use_for_npm_package(&package.id.name)
      || !initialized_file.exists()
    {
      let cache = cache.clone();
      let registry_url = registry_url.clone();
      let package = package.clone();
      let handle = tokio::task::spawn(async move {
        cache
          .ensure_package(
            (&package.id.name, &package.id.version),
            &package.dist,
            &registry_url,
          )
          .await?;
        let sub_node_modules = folder_path.join("node_modules");
        let package_path =
          join_package_name(&sub_node_modules, &package.id.name);
        fs::create_dir_all(&package_path)
          .with_context(|| format!("Creating '{}'", folder_path.display()))?;
        let cache_folder = cache.package_folder_for_name_and_version(
          &package.id.name,
          &package.id.version,
          &registry_url,
        );
        // for now copy, but in the future consider hard linking
        copy_dir_recursive(&cache_folder, &package_path)?;
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

  // 2. Create any "copy" packages, which are used for peer dependencies
  for package in &package_partitions.copy_packages {
    let package_cache_folder_id = package.get_package_cache_folder_id();
    let destination_path = deno_local_registry_dir
      .join(get_package_folder_id_folder_name(&package_cache_folder_id));
    let initialized_file = destination_path.join(".initialized");
    if !initialized_file.exists() {
      let sub_node_modules = destination_path.join("node_modules");
      let package_path = join_package_name(&sub_node_modules, &package.id.name);
      fs::create_dir_all(&package_path).with_context(|| {
        format!("Creating '{}'", destination_path.display())
      })?;
      let source_path = join_package_name(
        &deno_local_registry_dir
          .join(get_package_folder_id_folder_name(
            &package_cache_folder_id.with_no_count(),
          ))
          .join("node_modules"),
        &package.id.name,
      );
      hard_link_dir_recursive(&source_path, &package_path)?;
      // write out a file that indicates this folder has been initialized
      fs::write(initialized_file, "")?;
    }
  }

  let all_packages = package_partitions.into_all();

  // 3. Symlink all the dependencies into the .deno directory.
  //
  // Symlink node_modules/.deno/<package_id>/node_modules/<dep_name> to
  // node_modules/.deno/<dep_id>/node_modules/<dep_package_name>
  for package in &all_packages {
    let sub_node_modules = deno_local_registry_dir
      .join(get_package_folder_id_folder_name(
        &package.get_package_cache_folder_id(),
      ))
      .join("node_modules");
    for (name, dep_id) in &package.dependencies {
      let dep_cache_folder_id = snapshot
        .package_from_id(dep_id)
        .unwrap()
        .get_package_cache_folder_id();
      let dep_folder_name =
        get_package_folder_id_folder_name(&dep_cache_folder_id);
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

  // 4. Create all the packages in the node_modules folder, which are symlinks.
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
      package_id.display()
    } else {
      continue; // skip, already handled
    };
    let package = snapshot.package_from_id(&package_id).unwrap();
    let local_registry_package_path = join_package_name(
      &deno_local_registry_dir
        .join(get_package_folder_id_folder_name(
          &package.get_package_cache_folder_id(),
        ))
        .join("node_modules"),
      &package_id.name,
    );

    symlink_package_dir(
      &local_registry_package_path,
      &join_package_name(root_node_modules_dir_path, &root_folder_name),
    )?;
    for id in package.dependencies.values() {
      pending_packages.push_back((id.clone(), false));
    }
  }

  Ok(())
}

fn get_package_folder_id_folder_name(id: &NpmPackageCacheFolderId) -> String {
  let copy_str = if id.copy_index == 0 {
    "".to_string()
  } else {
    format!("_{}", id.copy_index)
  };
  let name = if id.name.to_lowercase() == id.name {
    Cow::Borrowed(&id.name)
  } else {
    Cow::Owned(format!("_{}", mixed_case_package_name_encode(&id.name)))
  };
  format!("{}@{}{}", name, id.version, copy_str).replace('/', "+")
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
  symlink_dir(old_path, new_path)
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

      match symlink_dir(old_path, new_path) {
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
