// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::url::Url;

use crate::npm::resolution::NpmResolution;
use crate::npm::resolvers::common::cache_packages;
use crate::npm::NpmCache;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmRegistryApi;

use super::common::InnerNpmPackageResolver;
use super::common::LocalNpmPackageInfo;

#[derive(Debug, Clone)]
pub struct GlobalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  registry_url: Url,
}

impl GlobalNpmPackageResolver {
  pub fn new(cache: NpmCache, api: NpmRegistryApi) -> Self {
    let registry_url = api.base_url().to_owned();
    let resolution = Arc::new(NpmResolution::new(api));

    Self {
      cache,
      resolution,
      registry_url,
    }
  }

  fn local_package_info(&self, id: &NpmPackageId) -> LocalNpmPackageInfo {
    LocalNpmPackageInfo {
      folder_path: self.cache.package_folder(id, &self.registry_url),
      id: id.clone(),
    }
  }
}

impl InnerNpmPackageResolver for GlobalNpmPackageResolver {
  fn resolve_package_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    let pkg = self.resolution.resolve_package_from_deno_module(pkg_req)?;
    Ok(self.local_package_info(&pkg.id))
  }

  fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    let referrer_pkg_id = self
      .cache
      .resolve_package_id_from_specifier(referrer, &self.registry_url)?;
    let pkg = self
      .resolution
      .resolve_package_from_package(name, &referrer_pkg_id)?;
    Ok(self.local_package_info(&pkg.id))
  }

  fn resolve_package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    let pkg_id = self
      .cache
      .resolve_package_id_from_specifier(specifier, &self.registry_url)?;
    Ok(self.local_package_info(&pkg_id))
  }

  fn has_packages(&self) -> bool {
    self.resolution.has_packages()
  }

  fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> BoxFuture<'static, Result<(), AnyError>> {
    let resolver = self.clone();
    async move {
      resolver.resolution.add_package_reqs(packages).await?;
      cache_packages(
        resolver.resolution.all_packages(),
        &resolver.cache,
        &resolver.registry_url,
      )
      .await
    }
    .boxed()
  }

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError> {
    let registry_path = self.cache.registry_folder(&self.registry_url);
    ensure_read_permission(&registry_path, path)
  }
}

fn ensure_read_permission(
  registry_path: &Path,
  path: &Path,
) -> Result<(), AnyError> {
  // allow reading if it's in the deno_dir node modules
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
