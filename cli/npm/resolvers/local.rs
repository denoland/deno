// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
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
use super::LocalNpmPackageInfo;

#[derive(Debug, Clone)]
pub struct LocalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  folder: Arc<RwLock<NodeModulesFolder>>,
  registry_url: Url,
  node_modules_folder: PathBuf,
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
      folder: Default::default(),
      registry_url,
      node_modules_folder,
    }
  }
}

impl InnerNpmPackageResolver for LocalNpmPackageResolver {
  fn resolve_package_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    todo!()
  }

  fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    todo!()
  }

  fn resolve_package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    todo!()
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

      let folder = setup_node_modules(
        &resolver.resolution.snapshot(),
        &resolver.cache,
        &resolver.registry_url,
        &resolver.node_modules_folder,
      )?;

      // todo(dsherret): move instead of clone
      *resolver.folder.write() = folder.read().clone();
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
  output_dir: &PathBuf,
) -> Result<Arc<RwLock<NodeModulesFolder>>, AnyError> {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let folder = create_virtual_node_modules_folder(snapshot);

  // todo(dsherret): ensure only one process enters this at a time.
  sync_folder_with_fs(&folder.read(), cache, registry_url, &output_dir)?;

  Ok(folder)
}

#[derive(Default, Debug, Clone)]
struct NodeModulesFolder {
  parent: Option<Arc<RwLock<NodeModulesPackage>>>,
  folders: HashMap<String, Arc<RwLock<NodeModulesPackage>>>,
  packages: HashMap<NpmPackageId, String>,
}

#[derive(Debug)]
struct NodeModulesPackage {
  id: NpmPackageId,
  parent: Arc<RwLock<NodeModulesFolder>>,
  child_folder: Arc<RwLock<NodeModulesFolder>>,
}

fn create_virtual_node_modules_folder(
  snapshot: &NpmResolutionSnapshot,
) -> Arc<RwLock<NodeModulesFolder>> {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let folder = Arc::new(RwLock::new(NodeModulesFolder::default()));

  // go over all the top level packages to ensure they're
  // kept in the top level folder
  for package in &top_level_packages {
    let folder_package = NodeModulesPackage {
      parent: folder.clone(),
      child_folder: Default::default(),
      id: package.id.clone(),
    };
    let mut folder = folder.write();
    let folder_name = if folder.folders.contains_key(&folder_package.id.name) {
      // This is when you say have two packages like so:
      //   import chalkv4 from "npm:chalk@4"
      //   import chalkv5 from "npm:chalk@5"
      // In this scenario, we use the full resolved package id
      // for the second package.
      folder_package.id.to_string()
    } else {
      folder_package.id.name.to_string()
    };
    folder
      .packages
      .insert(folder_package.id.clone(), folder_name.clone());
    let past_item = folder
      .folders
      .insert(folder_name, Arc::new(RwLock::new(folder_package)));
    assert!(past_item.is_none());
  }

  // now go over each package and sub packages and populate them in the folder
  for top_level_package in &top_level_packages {
    let node_modules_package = {
      let folder = folder.read();
      let folder_name = folder.packages.get(&top_level_package.id).unwrap();
      folder.folders.get(folder_name).unwrap().clone()
    };
    populate_folder_deps(top_level_package, &node_modules_package, snapshot);
  }
  folder
}

fn populate_folder_deps(
  package: &NpmResolutionPackage,
  node_modules_package: &RwLock<NodeModulesPackage>,
  snapshot: &NpmResolutionSnapshot,
) {
  // package_ref_name is what the package refers to the other package as
  for (package_ref_name, id) in &package.dependencies {
    if let Some(insert_folder) = get_insert_folder(
      package_ref_name,
      id,
      node_modules_package.read().child_folder.clone(),
    ) {
      let node_modules_package = Arc::new(RwLock::new(NodeModulesPackage {
        id: id.clone(),
        parent: insert_folder.clone(),
        child_folder: Default::default(),
      }));
      let mut insert_folder = insert_folder.write();
      insert_folder
        .packages
        .insert(id.clone(), package_ref_name.clone());
      let past_item = insert_folder
        .folders
        .insert(package_ref_name.clone(), node_modules_package.clone());
      assert!(past_item.is_none());

      // now go through all this module's dependencies
      let package = snapshot.package_from_id(id).unwrap();
      populate_folder_deps(&package, &node_modules_package, snapshot);
    }
  }
}

fn get_insert_folder(
  package_ref_name: &String,
  id: &NpmPackageId,
  child_folder: Arc<RwLock<NodeModulesFolder>>,
) -> Option<Arc<RwLock<NodeModulesFolder>>> {
  let mut current_folder = child_folder.clone();
  loop {
    let parent = {
      let folder = current_folder.read();
      if folder.folders.contains_key(package_ref_name) {
        if folder.packages.get(id) == Some(package_ref_name) {
          // same name and id exists in the tree, so ignore
          return None;
        } else {
          // same name, but different id exists in the tree, so
          // use the child folder
          return Some(child_folder);
        }
      }
      folder.parent.clone()
    };
    match parent {
      Some(parent) => {
        // go up to the parent folder
        current_folder = parent.read().parent.clone();
      }
      None => {
        // no name found, so insert in the root folder, which is the current folder
        return Some(current_folder);
      }
    }
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
  folder: &NodeModulesFolder,
  cache: &NpmCache,
  registry_url: &Url,
  output_dir: &PathBuf,
) -> Result<(), AnyError> {
  // create the folders
  fs::create_dir_all(output_dir)?;
  let resolution_file = output_dir.join(".deno_resolution");
  let mut folder_data: FolderData = fs::read_to_string(&resolution_file)
    .ok()
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default();
  for (folder_name, package) in &folder.folders {
    let local_folder = output_dir.join(folder_name);
    let package = package.read();
    let child_folder = package.child_folder.read();
    let cache_folder = cache.package_folder(&package.id, registry_url);
    let folder_kind = if child_folder.folders.is_empty() {
      FolderKind::Symlink
    } else {
      FolderKind::SubDir
    };
    let expected_folder_data_package = FolderDataPackage {
      id: package.id.to_string(),
      kind: folder_kind,
    };
    let state_matches = folder_data.packages.get(&package.id.name)
      == Some(&expected_folder_data_package);

    if !state_matches {
      match folder_kind {
        FolderKind::Symlink => {
          remove_dir_all(&local_folder)?;
          // no sub packages, so create a symlink
          symlink_dir(&cache_folder, &local_folder)?;
        }
        FolderKind::SubDir => {
          // there's sub packages, so symlink the children
          symlink_dir_children(&cache_folder, &local_folder)?;
        }
      }
      folder_data
        .packages
        .insert(package.id.name.to_string(), expected_folder_data_package);
    }

    if folder_kind == FolderKind::SubDir {
      let sub_folder = local_folder.join("node_modules");
      sync_folder_with_fs(&child_folder, cache, registry_url, &sub_folder)?;
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
