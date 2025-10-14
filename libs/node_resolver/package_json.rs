// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonCacheResult;
use deno_package_json::PackageJsonRc;
use sys_traits::FsMetadata;
use sys_traits::FsRead;

use crate::errors::PackageJsonLoadError;

pub trait NodePackageJsonCache:
  deno_package_json::PackageJsonCache
  + std::fmt::Debug
  + deno_maybe_sync::MaybeSend
  + deno_maybe_sync::MaybeSync
{
  fn as_deno_package_json_cache(
    &self,
  ) -> &dyn deno_package_json::PackageJsonCache;
}

impl<T> NodePackageJsonCache for T
where
  T: deno_package_json::PackageJsonCache
    + std::fmt::Debug
    + deno_maybe_sync::MaybeSend
    + deno_maybe_sync::MaybeSync,
{
  fn as_deno_package_json_cache(
    &self,
  ) -> &dyn deno_package_json::PackageJsonCache {
    self
  }
}

#[allow(clippy::disallowed_types)]
pub type PackageJsonCacheRc =
  deno_maybe_sync::MaybeArc<dyn NodePackageJsonCache>;

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
  fn get(&self, path: &Path) -> PackageJsonCacheResult {
    CACHE.with_borrow(|cache| match cache.get(path).cloned() {
      Some(value) => PackageJsonCacheResult::Hit(Some(value)),
      None => PackageJsonCacheResult::NotCached,
    })
  }

  fn set(&self, path: PathBuf, package_json: Option<PackageJsonRc>) {
    let Some(package_json) = package_json else {
      // We don't cache misses.
      return;
    };
    CACHE.with_borrow_mut(|cache| cache.insert(path, package_json));
  }
}

#[allow(clippy::disallowed_types)]
pub type PackageJsonResolverRc<TSys> =
  deno_maybe_sync::MaybeArc<PackageJsonResolver<TSys>>;

#[derive(Debug)]
pub struct PackageJsonResolver<TSys: FsRead + FsMetadata> {
  sys: TSys,
  loader_cache: Option<PackageJsonCacheRc>,
}

impl<TSys: FsRead + FsMetadata> PackageJsonResolver<TSys> {
  pub fn new(sys: TSys, loader_cache: Option<PackageJsonCacheRc>) -> Self {
    Self { sys, loader_cache }
  }

  pub fn get_closest_package_json(
    &self,
    file_path: &Path,
  ) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
    self.get_closest_package_jsons(file_path).next().transpose()
  }

  /// Gets the closest package.json files, iterating from the
  /// nearest directory to the furthest ancestor directory.
  pub fn get_closest_package_jsons<'a>(
    &'a self,
    file_path: &'a Path,
  ) -> ClosestPackageJsonsIterator<'a, TSys> {
    ClosestPackageJsonsIterator {
      current_path: file_path,
      resolver: self,
    }
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
      Ok(pkg_json) => Ok(pkg_json),
      Err(err) => Err(PackageJsonLoadError(err)),
    }
  }
}

pub struct ClosestPackageJsonsIterator<'a, TSys: FsRead + FsMetadata> {
  current_path: &'a Path,
  resolver: &'a PackageJsonResolver<TSys>,
}

impl<'a, TSys: FsRead + FsMetadata> Iterator
  for ClosestPackageJsonsIterator<'a, TSys>
{
  type Item = Result<PackageJsonRc, PackageJsonLoadError>;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(parent) = self.current_path.parent() {
      self.current_path = parent;
      let package_json_path = parent.join("package.json");
      match self.resolver.load_package_json(&package_json_path) {
        Ok(Some(value)) => return Some(Ok(value)),
        Ok(None) => {
          // skip
        }
        Err(err) => return Some(Err(err)),
      }
    }
    None
  }
}
