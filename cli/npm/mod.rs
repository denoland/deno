// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod cache;
mod registry;
mod resolution;
mod tarball;

use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;

use deno_core::futures;
use deno_core::url::Url;
pub use resolution::NpmPackageId;
pub use resolution::NpmPackageReference;
pub use resolution::NpmPackageReq;
pub use resolution::NpmResolutionPackage;

use cache::NpmCache;
use registry::NpmPackageVersionDistInfo;
use registry::NpmRegistryApi;
use resolution::NpmResolution;

use crate::deno_dir::DenoDir;

use self::cache::ReadonlyNpmCache;
use self::resolution::NpmResolutionSnapshot;

/// Information about the local npm package.
pub struct LocalNpmPackageInfo {
  /// Unique identifier.
  pub id: NpmPackageId,
  /// Local folder path of the npm package.
  pub folder_path: PathBuf,
}

pub trait NpmPackageResolver {
  /// Resolves an npm package from a Deno module.
  fn resolve_package_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  /// Resolves an npm package from an npm package referrer.
  fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  /// Resolve the root folder of the package the provided specifier is in.
  ///
  /// This will erorr when the provided specifier is not in an npm package.
  fn resolve_package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  /// Gets if the provided specifier is in an npm package.
  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self.resolve_package_from_specifier(specifier).is_ok()
  }
}

#[derive(Clone, Debug)]
pub struct GlobalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  registry_url: Url,
}

impl GlobalNpmPackageResolver {
  pub fn new(root_cache_dir: PathBuf, reload: bool) -> Self {
    Self::from_cache(NpmCache::new(root_cache_dir), reload)
  }

  pub fn from_deno_dir(dir: &DenoDir, reload: bool) -> Self {
    Self::from_cache(NpmCache::from_deno_dir(dir), reload)
  }

  fn from_cache(cache: NpmCache, reload: bool) -> Self {
    let api = NpmRegistryApi::new(cache.clone(), reload);
    let registry_url = api.base_url().to_owned();
    let resolution = Arc::new(NpmResolution::new(api));

    Self {
      cache,
      resolution,
      registry_url,
    }
  }

  /// If the resolver has resolved any npm packages.
  pub fn has_packages(&self) -> bool {
    self.resolution.has_packages()
  }

  /// Gets all the packages.
  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.resolution.all_packages()
  }

  /// Adds a package requirement to the resolver.
  pub async fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    self.resolution.add_package_reqs(packages).await
  }

  /// Caches all the packages in parallel.
  pub async fn cache_packages(&self) -> Result<(), AnyError> {
    let handles = self.resolution.all_packages().into_iter().map(|package| {
      let cache = self.cache.clone();
      let registry_url = self.registry_url.clone();
      tokio::task::spawn(async move {
        cache
          .ensure_package(&package.id, &package.dist, &registry_url)
          .await
          .with_context(|| {
            format!("Failed caching npm package '{}'.", package.id)
          })
      })
    });
    let results = futures::future::join_all(handles).await;
    for result in results {
      // surface the first error
      result??;
    }
    Ok(())
  }

  fn local_package_info(&self, id: &NpmPackageId) -> LocalNpmPackageInfo {
    LocalNpmPackageInfo {
      folder_path: self.cache.package_folder(id, &self.registry_url),
      id: id.clone(),
    }
  }

  /// Creates an inner clone.
  pub fn snapshot(&self) -> NpmPackageResolverSnapshot {
    NpmPackageResolverSnapshot {
      cache: self.cache.as_readonly(),
      snapshot: self.resolution.snapshot(),
      registry_url: self.registry_url.clone(),
    }
  }
}

impl NpmPackageResolver for GlobalNpmPackageResolver {
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
}

#[derive(Clone, Debug)]
pub struct NpmPackageResolverSnapshot {
  cache: ReadonlyNpmCache,
  snapshot: NpmResolutionSnapshot,
  registry_url: Url,
}

// todo(dsherret): implementing Default for this is error prone, but
// necessary for the LSP. We should remove this Default implementation.
// See comment on `ReadonlyNpmCache` for more details.
impl Default for NpmPackageResolverSnapshot {
  fn default() -> Self {
    Self {
      cache: Default::default(),
      snapshot: Default::default(),
      registry_url: NpmRegistryApi::default_url(),
    }
  }
}

impl NpmPackageResolverSnapshot {
  fn local_package_info(&self, id: &NpmPackageId) -> LocalNpmPackageInfo {
    LocalNpmPackageInfo {
      folder_path: self.cache.package_folder(id, &self.registry_url),
      id: id.clone(),
    }
  }
}

impl NpmPackageResolver for NpmPackageResolverSnapshot {
  fn resolve_package_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<LocalNpmPackageInfo, AnyError> {
    let pkg = self.snapshot.resolve_package_from_deno_module(pkg_req)?;
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
      .snapshot
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
}
