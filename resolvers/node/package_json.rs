// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonRc;
use sys_traits::FsRead;
use url::Url;

use crate::errors::ClosestPkgJsonError;
use crate::errors::PackageJsonLoadError;

// it would be nice if this was passed down as a ctor arg to the package.json resolver,
// but it's a little bit complicated to do that, so we just maintain a thread local cache
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
pub type PackageJsonResolverRc<TSys> =
  crate::sync::MaybeArc<PackageJsonResolver<TSys>>;

#[derive(Debug)]
pub struct PackageJsonResolver<TSys: FsRead> {
  sys: TSys,
}

impl<TSys: FsRead> PackageJsonResolver<TSys> {
  pub fn new(sys: TSys) -> Self {
    Self { sys }
  }

  pub fn get_closest_package_json(
    &self,
    url: &Url,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    let Ok(file_path) = deno_path_util::url_to_file_path(url) else {
      return Ok(None);
    };
    self.get_closest_package_json_from_file_path(&file_path)
  }

  pub fn get_closest_package_json_from_file_path(
    &self,
    file_path: &Path,
  ) -> Result<Option<PackageJsonRc>, ClosestPkgJsonError> {
    let parent_dir = file_path.parent().unwrap();
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
      Some(&PackageJsonThreadLocalCache),
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
