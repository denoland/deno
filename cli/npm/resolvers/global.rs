// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! Code for global npm cache resolution.

use std::path::Path;
use std::path::PathBuf;
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

use super::common::ensure_registry_read_permission;
use super::common::InnerNpmPackageResolver;

/// Resolves packages from the global npm cache.
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

  fn package_folder(&self, id: &NpmPackageId) -> PathBuf {
    self.cache.package_folder(id, &self.registry_url)
  }
}

impl InnerNpmPackageResolver for GlobalNpmPackageResolver {
  fn resolve_package_folder_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<PathBuf, AnyError> {
    let pkg = self.resolution.resolve_package_from_deno_module(pkg_req)?;
    Ok(self.package_folder(&pkg.id))
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let referrer_pkg_id = self
      .cache
      .resolve_package_id_from_specifier(referrer, &self.registry_url)?;
    let pkg = self
      .resolution
      .resolve_package_from_package(name, &referrer_pkg_id)?;
    Ok(self.package_folder(&pkg.id))
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    let pkg_id = self
      .cache
      .resolve_package_id_from_specifier(specifier, &self.registry_url)?;
    Ok(self.package_folder(&pkg_id))
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
    ensure_registry_read_permission(&registry_path, path)
  }
}
