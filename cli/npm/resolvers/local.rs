// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::RwLock;
use deno_core::serde_json;
use deno_core::url::Url;
use serde::Deserialize;
use serde::Serialize;

use crate::npm::resolution::NpmResolution;
use crate::npm::resolution::NpmResolutionSnapshot;
use crate::npm::NpmCache;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmRegistryApi;
use crate::npm::NpmResolutionPackage;

use super::common::cache_packages;
use super::common::ensure_registry_read_permission;
use super::common::InnerNpmPackageResolver;

#[derive(Debug, Clone)]
pub struct LocalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  folder_index: Arc<RwLock<NodeModulesFolderIndex>>,
  registry_url: Url,
  root_node_modules_path: PathBuf,
  root_node_modules_specifier: ModuleSpecifier,
}

impl LocalNpmPackageResolver {
  pub fn new(
    cache: NpmCache,
    api: NpmRegistryApi,
    node_modules_folder: PathBuf,
  ) -> Self {
    let registry_url = api.base_url().to_owned();
    let resolution = Arc::new(NpmResolution::new(api));

    Self {
      cache,
      resolution,
      folder_index: Default::default(),
      registry_url,
      root_node_modules_specifier: ModuleSpecifier::from_directory_path(&node_modules_folder).unwrap(),
      root_node_modules_path: node_modules_folder,
    }
  }

  fn resolve_package_root(&self, path: &Path) -> PathBuf {
    let mut last_found = path;
    loop {
      let parent = path.parent().unwrap();
      if parent.file_name().unwrap() == "node_modules" {
        return last_found.to_path_buf();
      } else {
        last_found = parent;
      }
    }
  }

  fn resolve_folder_for_specifier(&self, specifier: &ModuleSpecifier) -> Result<PathBuf, AnyError> {
    match self.maybe_resolve_folder_for_specifier(specifier) {
      Some(path) => Ok(path),
      None => bail!("could not find npm package for '{}'", specifier),
    }
  }

  fn maybe_resolve_folder_for_specifier(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    let relative_url = self.root_node_modules_specifier.make_relative(specifier)?;
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
    let folder_index = self.folder_index.read();
    let root_folder = folder_index.all_folders.get(&self.root_node_modules_path).unwrap();
    let sub_dir_name = if root_folder
      .folder_names
      .contains(&resolved_package.id.to_string())
    {
      // it's the fully resolved package name
      resolved_package.id.to_string()
    } else {
      resolved_package.id.name.clone()
    };
    Ok(self.root_node_modules_path.join(sub_dir_name))
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let local_path = self.resolve_folder_for_specifier(referrer)?;
    let package_root_path = self.resolve_package_root(&local_path);
    self.resolution.resolve_package_from_package(name, referrer)
    todo!()
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
      cache_packages(
        resolver.resolution.all_packages(),
        &resolver.cache,
        &resolver.registry_url,
      )
      .await?;

      let folder_index = setup_node_modules(
        &resolver.resolution.snapshot(),
        &resolver.cache,
        &resolver.registry_url,
        &resolver.root_node_modules_path,
      )?;

      *resolver.folder_index.write() = folder_index;
      Ok(())
    }
    .boxed()
  }

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError> {
    let registry_path = self.cache.registry_folder(&self.registry_url);
    ensure_registry_read_permission(&registry_path, path)
  }
}

fn setup_node_modules(
  snapshot: &NpmResolutionSnapshot,
  cache: &NpmCache,
  registry_url: &Url,
  root_node_modules_dir_path: &PathBuf,
) -> Result<NodeModulesFolderIndex, AnyError> {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let folders_index = create_virtual_node_modules_folder(snapshot, root_node_modules_dir_path);

  // todo(dsherret): ensure only one process enters this at a time.
  sync_folder_with_fs(&folders_index, &root_node_modules_dir_path, cache, registry_url)?;

  Ok(folders_index)
}

#[derive(Default, Debug)]
struct NodeModulesFolderIndex {
  // all folders based on their path
  all_folders: HashMap<PathBuf, NodeModulesFolder>,
}

#[derive(Default, Debug, Clone)]
struct NodeModulesFolder {
  path: PathBuf,
  folder_names: HashSet<String>,
  packages_to_folder_names: HashMap<NpmPackageId, String>,
}

impl NodeModulesFolder {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      ..Default::default()
    }
  }

  pub fn add_folder(&mut self, folder_name: String, package_id: NpmPackageId) {
    self.folder_names.insert(folder_name.clone());
    self.packages_to_folder_names.insert(package_id, folder_name);
  }
}

fn create_virtual_node_modules_folder(
  snapshot: &NpmResolutionSnapshot,
  root_node_modules_path: &Path,
) -> NodeModulesFolderIndex {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let mut folders_index = NodeModulesFolderIndex {
    all_folders: Default::default(),
  };
  let mut root_folder = NodeModulesFolder::new(root_node_modules_path.to_path_buf());

  // go over all the top level packages to ensure they're
  // kept in the top level folder
  for package in &top_level_packages {
    let folder_name = if root_folder.folder_names.contains(&package.id.name) {
      // This is when you say have two packages like so:
      //   import chalkv4 from "npm:chalk@4"
      //   import chalkv5 from "npm:chalk@5"
      // In this scenario, we use the full resolved package id
      // for the second package.
      package.id.to_string()
    } else {
      package.id.name.to_string()
    };
    root_folder.add_folder(folder_name.clone(), package.id.clone());
  }
  folders_index.all_folders.insert(root_node_modules_path.to_path_buf(), root_folder);

  // now go over each package and sub packages and populate them in the folder
  for top_level_package in &top_level_packages {
    let sub_node_modules_path = {
      let root_folder = folders_index.all_folders.get(root_node_modules_path).unwrap();
      let sub_dir = root_folder.packages_to_folder_names.get(&top_level_package.id).unwrap();
      root_folder.path.join(sub_dir).join("node_modules")
    };
    populate_folder_deps(top_level_package, &sub_node_modules_path, root_node_modules_path, &mut folders_index, snapshot);
  }
  folders_index
}

fn populate_folder_deps(
  package: &NpmResolutionPackage,
  sub_node_modules_path: &Path,
  root_node_modules_path: &Path,
  folders_index: &mut NodeModulesFolderIndex,
  snapshot: &NpmResolutionSnapshot,
) {
  // package_ref_name is what the package refers to the other package as
  for (package_ref_name, id) in &package.dependencies {
    if let Some(insert_folder) = get_insert_folder(
      package_ref_name,
      id,
      sub_node_modules_path,
      root_node_modules_path,
      folders_index,
    ) {
      let sub_node_modules_path = {
        let node_modules_folder = folders_index.all_folders.entry(insert_folder.clone()).or_insert_with(|| NodeModulesFolder::new(insert_folder));
        node_modules_folder.add_folder(package_ref_name.clone(), id.clone());
        node_modules_folder.path.join(package_ref_name).join("node_modules")
      };

      // now go through all this module's dependencies
      let package = snapshot.package_from_id(id).unwrap();

      populate_folder_deps(&package, &sub_node_modules_path, root_node_modules_path, folders_index, snapshot);
    }
  }
}

fn get_insert_folder(
  package_ref_name: &String,
  id: &NpmPackageId,
  sub_node_modules_path: &Path,
  root_node_modules_path: &Path,
  folders_index: &mut NodeModulesFolderIndex,
) -> Option<PathBuf> {
  let mut current_folder_path = sub_node_modules_path.to_path_buf();
  loop {
    if let Some(folder) = folders_index.all_folders.get(&current_folder_path) {
      if folder.folder_names.contains(package_ref_name) {
        if folder.packages_to_folder_names.get(id) == Some(package_ref_name) {
          // same name and id exists in the tree, so ignore
          return None;
        } else {
          // same name, but different id exists in the tree, so
          // use the child folder
          return Some(sub_node_modules_path.to_path_buf());
        }
      }
    }
    // go up two folders to the next node_modules
    let parent = sub_node_modules_path.parent().unwrap().parent().unwrap();
    if parent == root_node_modules_path {
      // no name found, so insert in the root folder
      return Some(root_node_modules_path.to_path_buf());
    }
    debug_assert_eq!(parent.file_stem().unwrap(), "node_modules");
    current_folder_path = parent.to_path_buf();
  }
}

#[derive(Default, Serialize, Deserialize)]
struct FolderData {
  packages: HashMap<String, FolderDataPackage>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct FolderDataPackage {
  id: String,
  kind: FolderKind,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
enum FolderKind {
  Symlink,
  SubDir,
}

fn sync_folder_with_fs(
  folders_index: &NodeModulesFolderIndex,
  output_dir: &PathBuf,
  cache: &NpmCache,
  registry_url: &Url,
) -> Result<(), AnyError> {
  let folder = match folders_index.all_folders.get(output_dir) {
    Some(folder) => folder,
    None => return Ok(()), // nothing to do
  };
  // create the folders
  fs::create_dir_all(output_dir)?;
  let resolution_file = output_dir.join(".deno_resolution");
  let mut folder_data: FolderData = fs::read_to_string(&resolution_file)
    .ok()
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default();
  for (package_id, folder_name) in &folder.packages_to_folder_names {
    let local_folder_path = output_dir.join(folder_name);
    let sub_node_modules_path = local_folder_path.join("node_modules");
    let cache_folder = cache.package_folder(&package_id, registry_url);
    let folder_kind = if folders_index.all_folders.get(&sub_node_modules_path).is_none() {
      FolderKind::Symlink
    } else {
      FolderKind::SubDir
    };
    let expected_folder_data_package = FolderDataPackage {
      id: package_id.to_string(),
      kind: folder_kind,
    };
    let state_matches = folder_data.packages.get(&package_id.name)
      == Some(&expected_folder_data_package);

    if !state_matches {
      match folder_kind {
        FolderKind::Symlink => {
          remove_dir_all(&local_folder_path)?;
          // no sub packages, so create a symlink
          symlink_dir(&cache_folder, &local_folder_path)?;
        }
        FolderKind::SubDir => {
          // there's sub packages, so symlink the children
          symlink_dir_children(&cache_folder, &local_folder_path)?;
        }
      }
      folder_data
        .packages
        .insert(package_id.name.to_string(), expected_folder_data_package);
    }

    if folder_kind == FolderKind::SubDir {
      let sub_folder = local_folder_path.join("node_modules");
      sync_folder_with_fs(folders_index, &sub_folder, cache, registry_url)?;
    }
  }

  if let Ok(text) = serde_json::to_string(&folder_data) {
    let _ignore = fs::write(resolution_file, text);
  }

  Ok(())
}

fn remove_dir_all(path: &Path) -> Result<(), AnyError> {
  match fs::remove_dir_all(path) {
    Ok(_) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(err) => Err(err.into()),
  }
}

fn symlink_dir_children(
  oldpath: &Path,
  newpath: &Path,
) -> Result<(), AnyError> {
  debug_assert!(oldpath.is_dir());
  fs::create_dir(&newpath)?;
  for entry in fs::read_dir(oldpath)? {
    let entry = entry?;
    if entry.file_type()?.is_dir() {
      symlink_dir(&entry.path(), &newpath.join(entry.file_name()))?;
    } else {
      symlink_file(&entry.path(), &newpath.join(entry.file_name()))?;
    }
  }
  Ok(())
}

// todo(dsherret): try to consolidate these symlink_dir and symlink_file functions
fn symlink_dir(oldpath: &Path, newpath: &Path) -> Result<(), AnyError> {
  use std::io::Error;
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::symlink;
    symlink(&oldpath, &newpath).map_err(err_mapper)?;
  }
  #[cfg(not(unix))]
  {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(&oldpath, &newpath).map_err(err_mapper)?;
  }
  Ok(())
}

fn symlink_file(oldpath: &Path, newpath: &Path) -> Result<(), AnyError> {
  use std::io::Error;
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::symlink;
    symlink(&oldpath, &newpath).map_err(err_mapper)?;
  }
  #[cfg(not(unix))]
  {
    use std::os::windows::fs::symlink_file;
    symlink_file(&oldpath, &newpath).map_err(err_mapper)?;
  }
  Ok(())
}
