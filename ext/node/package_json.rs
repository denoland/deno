// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_config::package_json::PackageJson;
use deno_config::package_json::PackageJsonLoadError;
use deno_config::package_json::PackageJsonRc;
use deno_fs::DenoConfigFsAdapter;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

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

impl deno_config::package_json::PackageJsonCache
  for PackageJsonThreadLocalCache
{
  fn get(&self, path: &Path) -> Option<PackageJsonRc> {
    CACHE.with(|cache| cache.borrow().get(path).cloned())
  }

  fn set(&self, path: PathBuf, package_json: PackageJsonRc) {
    CACHE.with(|cache| cache.borrow_mut().insert(path, package_json));
  }
}

/// Helper to load a package.json file using the thread local cache
/// in deno_node.
pub fn load_pkg_json(
  fs: &dyn deno_fs::FileSystem,
  path: &Path,
) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
  let result = PackageJson::load_from_path(
    path,
    &DenoConfigFsAdapter::new(fs),
    Some(&PackageJsonThreadLocalCache),
  );
  match result {
    Ok(pkg_json) => Ok(Some(pkg_json)),
    Err(PackageJsonLoadError::Io { source, .. })
      if source.kind() == ErrorKind::NotFound =>
    {
      Ok(None)
    }
    Err(err) => Err(err),
  }
}
