// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::BoxFuture;
use deno_core::url::Url;

use crate::npm::cache::should_sync_download;
use crate::npm::resolution::NpmResolutionSnapshot;
use crate::npm::NpmCache;
use crate::npm::NpmPackageReq;
use crate::npm::NpmResolutionPackage;

pub trait InnerNpmPackageResolver: Send + Sync {
  fn resolve_package_folder_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError>;

  fn has_packages(&self) -> bool;

  fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> BoxFuture<'static, Result<(), AnyError>>;

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError>;

  fn snapshot(&self) -> NpmResolutionSnapshot;
}

/// Caches all the packages in parallel.
pub async fn cache_packages(
  mut packages: Vec<NpmResolutionPackage>,
  cache: &NpmCache,
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
    let handle = tokio::task::spawn(async move {
      cache
        .ensure_package(&package.id, &package.dist, &registry_url)
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

pub fn ensure_registry_read_permission(
  registry_path: &Path,
  path: &Path,
) -> Result<(), AnyError> {
  // allow reading if it's in the node_modules
  if path.starts_with(&registry_path)
    && path
      .components()
      .all(|c| !matches!(c, std::path::Component::ParentDir))
  {
    // todo(dsherret): cache this?
    if let Ok(registry_path) = std::fs::canonicalize(registry_path) {
      match std::fs::canonicalize(path) {
        Ok(path) if path.starts_with(registry_path) => {
          return Ok(());
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
          return Ok(());
        }
        _ => {} // ignore
      }
    }
  }

  Err(deno_core::error::custom_error(
    "PermissionDenied",
    format!("Reading {} is not allowed", path.display()),
  ))
}
