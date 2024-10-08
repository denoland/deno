// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! Code for global npm cache resolution.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::colors;
use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;

use crate::args::LifecycleScriptsConfig;
use crate::cache::FastInsecureHasher;

use super::super::cache::NpmCache;
use super::super::cache::TarballCache;
use super::super::resolution::NpmResolution;
use super::common::cache_packages;
use super::common::lifecycle_scripts::LifecycleScriptsStrategy;
use super::common::NpmPackageFsResolver;
use super::common::RegistryReadPermissionChecker;

/// Resolves packages from the global npm cache.
#[derive(Debug)]
pub struct GlobalNpmPackageResolver {
  cache: Arc<NpmCache>,
  tarball_cache: Arc<TarballCache>,
  resolution: Arc<NpmResolution>,
  system_info: NpmSystemInfo,
  registry_read_permission_checker: RegistryReadPermissionChecker,
  lifecycle_scripts: LifecycleScriptsConfig,
}

impl GlobalNpmPackageResolver {
  pub fn new(
    cache: Arc<NpmCache>,
    fs: Arc<dyn FileSystem>,
    tarball_cache: Arc<TarballCache>,
    resolution: Arc<NpmResolution>,
    system_info: NpmSystemInfo,
    lifecycle_scripts: LifecycleScriptsConfig,
  ) -> Self {
    Self {
      registry_read_permission_checker: RegistryReadPermissionChecker::new(
        fs,
        cache.root_folder(),
      ),
      cache,
      tarball_cache,
      resolution,
      system_info,
      lifecycle_scripts,
    }
  }
}

#[async_trait(?Send)]
impl NpmPackageFsResolver for GlobalNpmPackageResolver {
  fn root_dir_url(&self) -> &Url {
    self.cache.root_dir_url()
  }

  fn node_modules_path(&self) -> Option<&Path> {
    None
  }

  fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let folder_id = self
      .resolution
      .resolve_pkg_cache_folder_id_from_pkg_id(id)?;
    Some(self.cache.package_folder_for_id(&folder_id))
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    use deno_npm::resolution::PackageNotFoundFromReferrerError;
    let Some(referrer_cache_folder_id) = self
      .cache
      .resolve_package_folder_id_from_specifier(referrer)
    else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.clone(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    let resolve_result = self
      .resolution
      .resolve_package_from_package(name, &referrer_cache_folder_id);
    match resolve_result {
      Ok(pkg) => match self.maybe_package_folder(&pkg.id) {
        Some(folder) => Ok(folder),
        None => Err(
          PackageNotFoundError {
            package_name: name.to_string(),
            referrer: referrer.clone(),
            referrer_extra: Some(format!(
              "{} -> {}",
              referrer_cache_folder_id,
              pkg.id.as_serialized()
            )),
          }
          .into(),
        ),
      },
      Err(err) => match *err {
        PackageNotFoundFromReferrerError::Referrer(cache_folder_id) => Err(
          ReferrerNotFoundError {
            referrer: referrer.clone(),
            referrer_extra: Some(cache_folder_id.to_string()),
          }
          .into(),
        ),
        PackageNotFoundFromReferrerError::Package {
          name,
          referrer: cache_folder_id_referrer,
        } => Err(
          PackageNotFoundError {
            package_name: name,
            referrer: referrer.clone(),
            referrer_extra: Some(cache_folder_id_referrer.to_string()),
          }
          .into(),
        ),
      },
    }
  }

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageCacheFolderId>, AnyError> {
    Ok(
      self
        .cache
        .resolve_package_folder_id_from_specifier(specifier),
    )
  }

  async fn cache_packages(&self) -> Result<(), AnyError> {
    let package_partitions = self
      .resolution
      .all_system_packages_partitioned(&self.system_info);
    cache_packages(&package_partitions.packages, &self.tarball_cache).await?;

    // create the copy package folders
    for copy in package_partitions.copy_packages {
      self
        .cache
        .ensure_copy_package(&copy.get_package_cache_folder_id())?;
    }

    let mut lifecycle_scripts =
      super::common::lifecycle_scripts::LifecycleScripts::new(
        &self.lifecycle_scripts,
        GlobalLifecycleScripts::new(self, &self.lifecycle_scripts.root_dir),
      );
    for package in &package_partitions.packages {
      let package_folder = self.cache.package_folder_for_nv(&package.id.nv);
      lifecycle_scripts.add(package, Cow::Borrowed(&package_folder));
    }

    lifecycle_scripts.warn_not_run_scripts()?;

    Ok(())
  }

  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, AnyError> {
    self
      .registry_read_permission_checker
      .ensure_registry_read_permission(permissions, path)
  }
}

struct GlobalLifecycleScripts<'a> {
  resolver: &'a GlobalNpmPackageResolver,
  path_hash: u64,
}

impl<'a> GlobalLifecycleScripts<'a> {
  fn new(resolver: &'a GlobalNpmPackageResolver, root_dir: &Path) -> Self {
    let mut hasher = FastInsecureHasher::new_without_deno_version();
    hasher.write(root_dir.to_string_lossy().as_bytes());
    let path_hash = hasher.finish();
    Self {
      resolver,
      path_hash,
    }
  }

  fn warned_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    self
      .package_path(package)
      .join(format!(".scripts-warned-{}", self.path_hash))
  }
}

impl<'a> super::common::lifecycle_scripts::LifecycleScriptsStrategy
  for GlobalLifecycleScripts<'a>
{
  fn can_run_scripts(&self) -> bool {
    false
  }
  fn package_path(&self, package: &NpmResolutionPackage) -> PathBuf {
    self.resolver.cache.package_folder_for_nv(&package.id.nv)
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> std::result::Result<(), deno_core::anyhow::Error> {
    log::warn!("{} The following packages contained npm lifecycle scripts ({}) that were not executed:", colors::yellow("Warning"), colors::gray("preinstall/install/postinstall"));
    for (package, _) in packages {
      log::warn!("┠─ {}", colors::gray(format!("npm:{}", package.id.nv)));
    }
    log::warn!("┃");
    log::warn!(
      "┠─ {}",
      colors::italic("This may cause the packages to not work correctly.")
    );
    log::warn!("┠─ {}", colors::italic("Lifecycle scripts are only supported when using a `node_modules` directory."));
    log::warn!(
      "┠─ {}",
      colors::italic("Enable it in your deno config file:")
    );
    log::warn!("┖─ {}", colors::bold("\"nodeModulesDir\": \"auto\""));

    for (package, _) in packages {
      std::fs::write(self.warned_scripts_file(package), "")?;
    }
    Ok(())
  }

  fn did_run_scripts(
    &self,
    _package: &NpmResolutionPackage,
  ) -> std::result::Result<(), deno_core::anyhow::Error> {
    Ok(())
  }

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool {
    self.warned_scripts_file(package).exists()
  }

  fn has_run(&self, _package: &NpmResolutionPackage) -> bool {
    false
  }
}
