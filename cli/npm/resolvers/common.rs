// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::task::spawn;
use deno_core::url::Url;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;

use crate::npm::cache::should_sync_download;
use crate::npm::NpmCache;

/// Part of the resolution that interacts with the file system.
#[async_trait]
pub trait NpmPackageFsResolver: Send + Sync {
  /// Specifier for the root directory.
  fn root_dir_url(&self) -> &Url;

  /// The local node_modules folder if it is applicable to the implementation.
  fn node_modules_path(&self) -> Option<PathBuf>;

  fn package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, AnyError>;
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError>;

  async fn cache_packages(&self) -> Result<(), AnyError>;

  fn ensure_read_permission(
    &self,
    permissions: &dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError>;
}

#[derive(Debug)]
pub struct RegistryReadPermissionChecker {
  fs: Arc<dyn FileSystem>,
  cache: Mutex<HashMap<PathBuf, PathBuf>>,
  registry_path: PathBuf,
}

impl RegistryReadPermissionChecker {
  pub fn new(fs: Arc<dyn FileSystem>, registry_path: PathBuf) -> Self {
    Self {
      fs,
      registry_path,
      cache: Default::default(),
    }
  }

  pub fn ensure_registry_read_permission(
    &self,
    permissions: &dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    // allow reading if it's in the node_modules
    let is_path_in_node_modules = path.starts_with(&self.registry_path)
      && path
        .components()
        .all(|c| !matches!(c, std::path::Component::ParentDir));

    if is_path_in_node_modules {
      let mut cache = self.cache.lock().unwrap();
      let registry_path_canon = match cache.get(&self.registry_path) {
        Some(canon) => canon.clone(),
        None => {
          let canon = self.fs.realpath_sync(&self.registry_path)?;
          cache.insert(self.registry_path.to_path_buf(), canon.clone());
          canon
        }
      };

      let path_canon = match cache.get(path) {
        Some(canon) => canon.clone(),
        None => {
          let canon = self.fs.realpath_sync(path);

          if canon
            .as_ref()
            .is_err_and(|e| e.kind() == ErrorKind::NotFound)
          {
            return Ok(());
          }

          let canon = canon?;
          cache.insert(path.to_path_buf(), canon.clone());
          canon
        }
      };

      if path_canon.starts_with(&registry_path_canon) {
        return Ok(());
      }
    }

    permissions.check_read(path)
  }
}

/// Caches all the packages in parallel.
pub async fn cache_packages(
  mut packages: Vec<NpmResolutionPackage>,
  cache: &Arc<NpmCache>,
  registry_url: &Url,
) -> Result<(), AnyError> {
  let sync_download = should_sync_download();
  if sync_download {
    // we're running the tests not with --quiet
    // and we want the output to be deterministic
    packages.sort_by(|a, b| a.id.cmp(&b.id));
  }

  let mut handles = Vec::with_capacity(packages.len());
  for package in packages {
    let cache = cache.clone();
    let registry_url = registry_url.clone();
    let handle = spawn(async move {
      cache
        .ensure_package(&package.id.nv, &package.dist, &registry_url)
        .await
    });
    if sync_download {
      handle.await??;
    } else {
      handles.push(handle);
    }
  }
  let results = futures::future::join_all(handles).await;
  for result in results {
    // surface the first error
    result??;
  }
  Ok(())
}

/// Gets the corresponding @types package for the provided package name.
pub fn types_package_name(package_name: &str) -> String {
  debug_assert!(!package_name.starts_with("@types/"));
  // Scoped packages will get two underscores for each slash
  // https://github.com/DefinitelyTyped/DefinitelyTyped/tree/15f1ece08f7b498f4b9a2147c2a46e94416ca777#what-about-scoped-packages
  format!("@types/{}", package_name.replace('/', "__"))
}

#[cfg(test)]
mod test {
  use super::types_package_name;

  #[test]
  fn test_types_package_name() {
    assert_eq!(types_package_name("name"), "@types/name");
    assert_eq!(
      types_package_name("@scoped/package"),
      "@types/@scoped__package"
    );
  }
}
