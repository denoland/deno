// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonRc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use crate::errors::PackageJsonLoadError;

// use a thread local cache so that workers have their own distinct cache
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

/// Helper to load a package.json file using the thread local cache
/// in node_resolver.
pub fn load_pkg_json(
  fs: &dyn deno_package_json::fs::DenoPkgJsonFs,
  path: &Path,
) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
  let result =
    PackageJson::load_from_path(path, fs, Some(&PackageJsonThreadLocalCache));
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
