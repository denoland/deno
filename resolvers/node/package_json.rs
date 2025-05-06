// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonRc;
use sys_traits::FsRead;

use crate::errors::ClosestPkgJsonError;
use crate::errors::PackageJsonLoadError;

pub trait NodePackageJsonCache:
  deno_package_json::PackageJsonCache
  + std::fmt::Debug
  + crate::sync::MaybeSend
  + crate::sync::MaybeSync
{
  fn as_deno_package_json_cache(
    &self,
  ) -> &dyn deno_package_json::PackageJsonCache;
}

impl<T> NodePackageJsonCache for T
where
  T: deno_package_json::PackageJsonCache
    + std::fmt::Debug
    + crate::sync::MaybeSend
    + crate::sync::MaybeSync,
{
  fn as_deno_package_json_cache(
    &self,
  ) -> &dyn deno_package_json::PackageJsonCache {
    self
  }
}

#[allow(clippy::disallowed_types)]
pub type PackageJsonCacheRc = crate::sync::MaybeArc<dyn NodePackageJsonCache>;

thread_local! {
  static CACHE: RefCell<HashMap<PathBuf, PackageJsonRc>> = RefCell::new(HashMap::new());
}

#[derive(Debug)]
pub struct PackageJsonThreadLocalCache;

impl PackageJsonThreadLocalCache {
  pub fn clear() {
    CACHE.with_borrow_mut(|cache| cache.clear());
  }
}

impl deno_package_json::PackageJsonCache for PackageJsonThreadLocalCache {
  fn get(&self, path: &Path) -> Option<PackageJsonRc> {
    CACHE.with_borrow(|cache| cache.get(path).cloned())
  }

  fn set(&self, path: PathBuf, package_json: PackageJsonRc) {
    CACHE.with_borrow_mut(|cache| cache.insert(path, package_json));
  }
}

#[allow(clippy::disallowed_types)]
pub type PackageJsonResolverRc<TSys> =
  crate::sync::MaybeArc<PackageJsonResolver<TSys>>;

#[derive(Debug)]
pub struct PackageJsonResolver<TSys: FsRead> {
  sys: TSys,
  loader_cache: Option<PackageJsonCacheRc>,
}

impl<TSys: FsRead> PackageJsonResolver<TSys> {
  pub fn new(sys: TSys, loader_cache: Option<PackageJsonCacheRc>) -> Self {
    Self { sys, loader_cache }
  }

  pub fn get_closest_package_json(
    &self,
    file_path: &Path,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    let Some(parent_dir) = file_path.parent() else {
      return Ok(None);
    };
    for current_dir in parent_dir.ancestors() {
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
      &self.sys,
      self
        .loader_cache
        .as_deref()
        .map(|cache| cache.as_deno_package_json_cache()),
      path,
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
