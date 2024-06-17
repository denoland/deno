// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_config::package_json::PackageJson;
use deno_config::package_json::PackageJsonLoadError;
use deno_fs::DenoConfigFsAdapter;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

thread_local! {
  static CACHE: RefCell<HashMap<PathBuf, Arc<PackageJson>>> = RefCell::new(HashMap::new());
}

pub struct PackageJsonThreadLocalCache;

impl deno_config::package_json::PackageJsonCache
  for PackageJsonThreadLocalCache
{
  fn get(&self, path: &Path) -> Option<Arc<PackageJson>> {
    CACHE.with(|cache| cache.borrow().get(path).cloned())
  }

  fn insert(&self, path: PathBuf, package_json: Arc<PackageJson>) {
    CACHE.with(|cache| cache.borrow_mut().insert(path, package_json));
  }
}

pub fn load_pkg_json(
  fs: &dyn deno_fs::FileSystem,
  path: &Path,
) -> Result<Option<Arc<PackageJson>>, PackageJsonLoadError> {
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
