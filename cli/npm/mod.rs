// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod cache;
mod registry;
mod resolution;
mod semver;
mod tarball;

use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::url::Url;
use deno_runtime::deno_node::DenoDirNpmResolver;
pub use resolution::NpmPackageId;
pub use resolution::NpmPackageReference;
pub use resolution::NpmPackageReq;
pub use resolution::NpmResolutionPackage;

use cache::NpmCache;
use registry::NpmPackageVersionDistInfo;
use registry::NpmRegistryApi;
use resolution::NpmResolution;

use crate::deno_dir::DenoDir;
use crate::file_fetcher::CacheSetting;

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
  /// This will error when the provided specifier is not in an npm package.
  fn resolve_package_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<LocalNpmPackageInfo, AnyError>;

  /// Gets if the provided specifier is in an npm package.
  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self.resolve_package_from_specifier(specifier).is_ok()
  }
}

#[derive(Debug, Clone)]
pub struct GlobalNpmPackageResolver {
  cache: NpmCache,
  resolution: Arc<NpmResolution>,
  registry_url: Url,
  unstable: bool,
  no_npm: bool,
}

impl GlobalNpmPackageResolver {
  pub fn from_deno_dir(
    dir: &DenoDir,
    reload: bool,
    cache_setting: CacheSetting,
    unstable: bool,
    no_npm: bool,
  ) -> Self {
    Self::from_cache(
      NpmCache::from_deno_dir(dir, cache_setting.clone()),
      reload,
      cache_setting,
      unstable,
      no_npm,
    )
  }

  fn from_cache(
    cache: NpmCache,
    reload: bool,
    cache_setting: CacheSetting,
    unstable: bool,
    no_npm: bool,
  ) -> Self {
    let api = NpmRegistryApi::new(cache.clone(), reload, cache_setting);
    let registry_url = api.base_url().to_owned();
    let resolution = Arc::new(NpmResolution::new(api));

    Self {
      cache,
      resolution,
      registry_url,
      unstable,
      no_npm,
    }
  }

  /// If the resolver has resolved any npm packages.
  pub fn has_packages(&self) -> bool {
    self.resolution.has_packages()
  }

  /// Adds a package requirement to the resolver.
  pub async fn add_package_reqs(
    &self,
    packages: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    assert!(!packages.is_empty());

    if !self.unstable {
      bail!(
        "Unstable use of npm specifiers. The --unstable flag must be provided."
      )
    }

    if self.no_npm {
      let fmt_reqs = packages
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ");
      return Err(custom_error(
        "NoNpm",
        format!(
          "Following npm specifiers were requested: {}; but --no-npm is specified.",
          fmt_reqs
        ),
      ));
    }
    self.resolution.add_package_reqs(packages).await
  }

  /// Caches all the packages in parallel.
  pub async fn cache_packages(&self) -> Result<(), AnyError> {
    if std::env::var("DENO_UNSTABLE_NPM_SYNC_DOWNLOAD") == Ok("1".to_string()) {
      // for some of the tests, we want downloading of packages
      // to be deterministic so that the output is always the same
      let mut packages = self.resolution.all_packages();
      packages.sort_by(|a, b| a.id.cmp(&b.id));
      for package in packages {
        self
          .cache
          .ensure_package(&package.id, &package.dist, &self.registry_url)
          .await
          .with_context(|| {
            format!("Failed caching npm package '{}'.", package.id)
          })?;
      }
    } else {
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
  #[allow(unused)]
  pub fn snapshot(&self) -> NpmPackageResolverSnapshot {
    NpmPackageResolverSnapshot {
      cache: self.cache.as_readonly(),
      snapshot: self.resolution.snapshot(),
      registry_url: self.registry_url.clone(),
    }
  }

  pub fn get_cache_location(&self) -> PathBuf {
    self.cache.as_readonly().get_cache_location()
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

impl DenoDirNpmResolver for GlobalNpmPackageResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &std::path::Path,
  ) -> Result<PathBuf, AnyError> {
    let referrer = specifier_to_path(referrer)?;
    self
      .resolve_package_from_package(specifier, &referrer)
      .map(|p| p.folder_path)
  }

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError> {
    let specifier = specifier_to_path(path)?;
    self
      .resolve_package_from_specifier(&specifier)
      .map(|p| p.folder_path)
  }

  fn in_npm_package(&self, path: &Path) -> bool {
    let specifier = match ModuleSpecifier::from_file_path(path) {
      Ok(p) => p,
      Err(_) => return false,
    };
    self.resolve_package_from_specifier(&specifier).is_ok()
  }

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError> {
    let registry_path = self.cache.registry_folder(&self.registry_url);
    ensure_read_permission(&registry_path, path)
  }
}

impl DenoDirNpmResolver for NpmPackageResolverSnapshot {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &std::path::Path,
  ) -> Result<PathBuf, AnyError> {
    let referrer = specifier_to_path(referrer)?;
    self
      .resolve_package_from_package(specifier, &referrer)
      .map(|p| p.folder_path)
  }

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError> {
    let specifier = specifier_to_path(path)?;
    self
      .resolve_package_from_specifier(&specifier)
      .map(|p| p.folder_path)
  }

  fn in_npm_package(&self, path: &Path) -> bool {
    let specifier = match ModuleSpecifier::from_file_path(path) {
      Ok(p) => p,
      Err(_) => return false,
    };
    self.resolve_package_from_specifier(&specifier).is_ok()
  }

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError> {
    let registry_path = self.cache.registry_folder(&self.registry_url);
    ensure_read_permission(&registry_path, path)
  }
}

fn specifier_to_path(path: &Path) -> Result<ModuleSpecifier, AnyError> {
  match ModuleSpecifier::from_file_path(&path) {
    Ok(specifier) => Ok(specifier),
    Err(()) => bail!("Could not convert '{}' to url.", path.display()),
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
