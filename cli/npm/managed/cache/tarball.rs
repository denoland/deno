// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_runtime::deno_fs::FileSystem;
use deno_semver::package::PackageNv;

use crate::args::CacheSetting;
use crate::http_util::HttpClientProvider;
use crate::npm::common::maybe_auth_header_for_npm_registry;
use crate::util::progress_bar::ProgressBar;

use super::tarball_extract::verify_and_extract_tarball;
use super::tarball_extract::TarballExtractionMode;
use super::NpmCache;

// todo(dsherret): create seams and unit test this

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't finished yet.
  PendingFuture(Shared<BoxFuture<'static, Result<(), Arc<AnyError>>>>),
  /// The result errored.
  Errored(Arc<AnyError>),
  /// This package has already been cached.
  Cached,
}

/// Coordinates caching of tarballs being loaded from
/// the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct TarballCache {
  cache: Arc<NpmCache>,
  fs: Arc<dyn FileSystem>,
  http_client_provider: Arc<HttpClientProvider>,
  npmrc: Arc<ResolvedNpmRc>,
  progress_bar: ProgressBar,
  memory_cache: Mutex<HashMap<PackageNv, MemoryCacheItem>>,
}

impl TarballCache {
  pub fn new(
    cache: Arc<NpmCache>,
    fs: Arc<dyn FileSystem>,
    http_client_provider: Arc<HttpClientProvider>,
    npmrc: Arc<ResolvedNpmRc>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      cache,
      fs,
      http_client_provider,
      npmrc,
      progress_bar,
      memory_cache: Default::default(),
    }
  }

  pub async fn ensure_package(
    &self,
    package: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), AnyError> {
    self
      .ensure_package_inner(package, dist)
      .await
      .with_context(|| format!("Failed caching npm package '{}'.", package))
  }

  async fn ensure_package_inner(
    &self,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), AnyError> {
    let (created, cache_item) = {
      let mut mem_cache = self.memory_cache.lock();
      if let Some(cache_item) = mem_cache.get(package_nv) {
        (false, cache_item.clone())
      } else {
        let future = self.create_setup_future(package_nv.clone(), dist.clone());
        let cache_item = MemoryCacheItem::PendingFuture(future);
        mem_cache.insert(package_nv.clone(), cache_item.clone());
        (true, cache_item)
      }
    };

    match cache_item {
      MemoryCacheItem::Cached => Ok(()),
      MemoryCacheItem::Errored(err) => Err(anyhow!("{}", err)),
      MemoryCacheItem::PendingFuture(future) => {
        if created {
          match future.await {
            Ok(_) => {
              *self.memory_cache.lock().get_mut(package_nv).unwrap() =
                MemoryCacheItem::Cached;
              Ok(())
            }
            Err(err) => {
              let result_err = anyhow!("{}", err);
              *self.memory_cache.lock().get_mut(package_nv).unwrap() =
                MemoryCacheItem::Errored(err);
              Err(result_err)
            }
          }
        } else {
          future.await.map_err(|err| anyhow!("{}", err))
        }
      }
    }
  }

  fn create_setup_future(
    &self,
    package_nv: PackageNv,
    dist: NpmPackageVersionDistInfo,
  ) -> Shared<BoxFuture<'static, Result<(), Arc<AnyError>>>> {
    let registry_url = self.npmrc.get_registry_url(&package_nv.name);
    let registry_config =
      self.npmrc.get_registry_config(&package_nv.name).clone();

    let cache = self.cache.clone();
    let fs = self.fs.clone();
    let progress_bar = self.progress_bar.clone();
    let package_folder =
      cache.package_folder_for_nv_and_url(&package_nv, registry_url);
    let http_client_provider = self.http_client_provider.clone();

    deno_core::unsync::spawn(async move {
      let should_use_cache = cache.should_use_cache_for_package(&package_nv);
      let package_folder_exists = fs.exists_sync(&package_folder);
      if should_use_cache && package_folder_exists {
        return Ok(());
      } else if cache.cache_setting() == &CacheSetting::Only {
        return Err(custom_error(
          "NotCached",
          format!(
            "An npm specifier not found in cache: \"{}\", --cached-only is specified.",
            &package_nv.name
          )
        )
        );
      }

      if dist.tarball.is_empty() {
        bail!("Tarball URL was empty.");
      }

      let maybe_auth_header =
        maybe_auth_header_for_npm_registry(&registry_config);

      let guard = progress_bar.update(&dist.tarball);
      let maybe_bytes = http_client_provider.get_or_create()?
        .download_with_progress(&dist.tarball, maybe_auth_header, &guard)
        .await?;
      match maybe_bytes {
        Some(bytes) => {
          let extraction_mode = if should_use_cache || !package_folder_exists {
            TarballExtractionMode::SiblingTempDir
          } else {
            // The user ran with `--reload`, so overwrite the package instead of
            // deleting it since the package might get corrupted if a user kills
            // their deno process while it's deleting a package directory
            //
            // We can't rename this folder and delete it because the folder
            // may be in use by another process or may now contain hardlinks,
            // which will cause windows to throw an "AccessDenied" error when
            // renaming. So we settle for overwriting.
            TarballExtractionMode::Overwrite
          };
          let dist = dist.clone();
          let package_nv = package_nv.clone();
          deno_core::unsync::spawn_blocking(move || {
            verify_and_extract_tarball(
              &package_nv,
              &bytes,
              &dist,
              &package_folder,
              extraction_mode,
            )
          })
          .await?
        }
        None => {
          bail!("Could not find npm package tarball at: {}", dist.tarball);
        }
      }
    })
      .map(|result| result.unwrap().map_err(Arc::new))
      .boxed()
      .shared()
  }
}
