// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonRc;
use deno_path_util::strip_unc_prefix;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

use crate::env::NodeResolverEnv;
use crate::errors::CanonicalizingPkgJsonDirError;
use crate::errors::ClosestPkgJsonError;
use crate::errors::PackageJsonLoadError;

// todo(dsherret): this isn't exactly correct and we should change it to instead
// be created per worker and passed down as a ctor arg to the pkg json resolver
thread_local! {
  static CACHE: RefCell<HashMap<PathBuf, PackageJsonRc>> = RefCell::new(HashMap::new());
}

pub struct PackageJsonThreadLocalCache;

impl PackageJsonThreadLocalCache {
  pub fn clear() {
    CACHE.with(|cache| cache.borrow_mut().clear());
  }
}

impl deno_package_json::PackageJsonCache for PackageJsonThreadLocalCache {
  fn get(&self, path: &Path) -> Option<PackageJsonRc> {
    CACHE.with(|cache| cache.borrow().get(path).cloned())
  }

  fn set(&self, path: PathBuf, package_json: PackageJsonRc) {
    CACHE.with(|cache| cache.borrow_mut().insert(path, package_json));
  }
}

#[allow(clippy::disallowed_types)]
pub type PackageJsonResolverRc<TEnv> =
  crate::sync::MaybeArc<PackageJsonResolver<TEnv>>;

#[derive(Debug)]
pub struct PackageJsonResolver<TEnv: NodeResolverEnv> {
  env: TEnv,
}

impl<TEnv: NodeResolverEnv> PackageJsonResolver<TEnv> {
  pub fn new(env: TEnv) -> Self {
    Self { env }
  }

  pub fn get_closest_package_json(
    &self,
    url: &Url,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    let Ok(file_path) = deno_path_util::url_to_file_path(url) else {
      return Ok(None);
    };
    self.get_closest_package_json_from_path(&file_path)
  }

  pub fn get_closest_package_json_from_path(
    &self,
    file_path: &Path,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    // we use this for deno compile using byonm because the script paths
    // won't be in virtual file system, but the package.json paths will be
    fn canonicalize_first_ancestor_exists<TEnv: NodeResolverEnv>(
      dir_path: &Path,
      env: &TEnv,
    ) -> Result<Option<PathBuf>, std::io::Error> {
      for ancestor in dir_path.ancestors() {
        match env.realpath_sync(ancestor) {
          Ok(dir_path) => return Ok(Some(dir_path)),
          Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // keep searching
          }
          Err(err) => return Err(err),
        }
      }
      Ok(None)
    }

    let parent_dir = file_path.parent().unwrap();
    let Some(start_dir) = canonicalize_first_ancestor_exists(
      parent_dir, &self.env,
    )
    .map_err(|source| CanonicalizingPkgJsonDirError {
      dir_path: parent_dir.to_path_buf(),
      source,
    })?
    else {
      return Ok(None);
    };
    let start_dir = strip_unc_prefix(start_dir);
    for current_dir in start_dir.ancestors() {
      let package_json_path = current_dir.join("package.json");
      if let Some(pkg_json) = self.load_package_json(&package_json_path)? {
        return Ok(Some(pkg_json));
      }
    }

    Ok(None)
  }

  pub fn load_package_json(
    &self,
    path: &Path,
  ) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
    let result = PackageJson::load_from_path(
      path,
      self.env.pkg_json_fs(),
      Some(&PackageJsonThreadLocalCache),
    );
    match result {
      Ok(pkg_json) => Ok(Some(pkg_json)),
      Err(deno_package_json::PackageJsonLoadError::Io { source, .. })
        if source.kind() == ErrorKind::NotFound =>
      {
        Ok(None)
      }
      Err(err) => Err(PackageJsonLoadError(err)),
    }
  }
}
