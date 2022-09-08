// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;

use super::cache::NpmCache;
use super::resolution::NpmResolutionSnapshot;
use super::NpmPackageId;
use super::NpmResolutionPackage;

#[derive(Default)]
struct NodeModulesFolder {
  parent: Option<Rc<RefCell<NodeModulesPackage>>>,
  folders: HashMap<String, Rc<RefCell<NodeModulesPackage>>>,
  packages: HashMap<NpmPackageId, String>,
}

struct NodeModulesPackage {
  id: NpmPackageId,
  parent: Rc<RefCell<NodeModulesFolder>>,
  child_folder: Rc<RefCell<NodeModulesFolder>>,
}

pub fn setup_node_modules(
  snapshot: &NpmResolutionSnapshot,
  cache: &NpmCache,
  registry_url: &Url,
  output_dir: PathBuf,
) -> Result<(), AnyError> {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let folder = create_virtual_node_modules_folder(snapshot);

  // todo:
  // - Need to ensure only one process enters this at a time.
  sync_folder_with_fs(&folder.borrow(), cache, registry_url, output_dir)?;
  Ok(())
}

fn create_virtual_node_modules_folder(
  snapshot: &NpmResolutionSnapshot,
) -> Rc<RefCell<NodeModulesFolder>> {
  // resolve everything to folder structure
  let mut top_level_packages = snapshot
    .top_level_packages()
    .into_iter()
    .map(|id| snapshot.package_from_id(&id).unwrap())
    .collect::<Vec<_>>();
  top_level_packages.sort_by(|a, b| a.id.cmp(&b.id));

  let folder = Rc::new(RefCell::new(NodeModulesFolder::default()));

  // go over all the top level packages to ensure they're
  // kept in the top level folder
  for package in &top_level_packages {
    let folder_package = NodeModulesPackage {
      parent: folder.clone(),
      child_folder: Default::default(),
      id: package.id.clone(),
    };
    let mut folder = folder.borrow_mut();
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
      .insert(folder_name, Rc::new(RefCell::new(folder_package)));
    assert!(past_item.is_none());
  }

  // now go over each package and sub packages and populate them in the folder
  for top_level_package in &top_level_packages {
    let node_modules_package = {
      let folder = RefCell::borrow(&folder);
      let folder_name = folder.packages.get(&top_level_package.id).unwrap();
      folder.folders.get(folder_name).unwrap().clone()
    };
    populate_folder_deps(top_level_package, &node_modules_package, snapshot);
  }
  folder
}

fn populate_folder_deps(
  package: &NpmResolutionPackage,
  node_modules_package: &RefCell<NodeModulesPackage>,
  snapshot: &NpmResolutionSnapshot,
) {
  // package_ref_name is what the package refers to the other package as
  for (package_ref_name, id) in &package.dependencies {
    if let Some(insert_folder) = get_insert_folder(
      package_ref_name,
      id,
      node_modules_package.borrow().child_folder.clone(),
    ) {
      let node_modules_package = Rc::new(RefCell::new(NodeModulesPackage {
        id: id.clone(),
        parent: insert_folder.clone(),
        child_folder: Default::default(),
      }));
      let mut insert_folder = insert_folder.borrow_mut();
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
  child_folder: Rc<RefCell<NodeModulesFolder>>,
) -> Option<Rc<RefCell<NodeModulesFolder>>> {
  let mut current_folder = child_folder.clone();
  loop {
    let parent = {
      let folder = current_folder.borrow();
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
        current_folder = parent.borrow().parent.clone();
      }
      None => {
        // no name found, so insert in the root folder, which is the current folder
        return Some(current_folder);
      }
    }
  }
}

fn sync_folder_with_fs(
  folder: &NodeModulesFolder,
  cache: &NpmCache,
  registry_url: &Url,
  output_dir: PathBuf,
) -> Result<(), AnyError> {
  // create the folders
  fs::create_dir_all(&output_dir)?;
  for (folder_name, package) in &folder.folders {
    let local_folder = output_dir.join(folder_name);
    remove_dir_all(&local_folder)?;
    let package = package.borrow();
    let child_folder = package.child_folder.borrow();
    let cache_folder = cache.package_folder(&package.id, registry_url);
    if child_folder.folders.is_empty() {
      // no sub packages, so create a symlink
      symlink_dir(&cache_folder, &local_folder)?;
    } else {
      // there's sub packages, so symlink the children
      symlink_dir_children(&cache_folder, &local_folder)?;
      let sub_folder = local_folder.join("node_modules");
      sync_folder_with_fs(&child_folder, cache, registry_url, sub_folder)?;
    }
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
