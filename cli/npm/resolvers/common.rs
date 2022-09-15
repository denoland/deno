use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::BoxFuture;
use deno_core::url::Url;

use crate::npm::NpmCache;
use crate::npm::NpmPackageId;
use crate::npm::NpmPackageReq;
use crate::npm::NpmResolutionPackage;

/// Information about the local npm package.
pub struct LocalNpmPackageInfo {
  /// Unique identifier.
  pub id: NpmPackageId,
  /// Local folder path of the npm package.
  pub folder_path: PathBuf,
}

pub trait InnerNpmPackageResolver: Send + Sync {
  fn resolve_package_from_deno_module(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  fn resolve_package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  fn has_packages(&self) -> bool;

  fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> BoxFuture<'static, Result<(), AnyError>>;

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError>;
}

/// Caches all the packages in parallel.
pub async fn cache_packages(
  mut packages: Vec<NpmResolutionPackage>,
  cache: &NpmCache,
  registry_url: &Url,
) -> Result<(), AnyError> {
  if std::env::var("DENO_UNSTABLE_NPM_SYNC_DOWNLOAD") == Ok("1".to_string()) {
    // for some of the tests, we want downloading of packages
    // to be deterministic so that the output is always the same
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    for package in packages {
      cache
        .ensure_package(&package.id, &package.dist, registry_url)
        .await
        .with_context(|| {
          format!("Failed caching npm package '{}'.", package.id)
        })?;
    }
  } else {
    let handles = packages.into_iter().map(|package| {
      let cache = cache.clone();
      let registry_url = registry_url.clone();
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
  }
  Ok(())
}
