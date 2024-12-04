// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error as AnyError;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_semver::package::PackageNv;
use deno_unsync::sync::MultiRuntimeAsyncValueCreator;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use http::StatusCode;
use parking_lot::Mutex;
use url::Url;

use crate::remote::maybe_auth_header_for_npm_registry;
use crate::tarball_extract::verify_and_extract_tarball;
use crate::tarball_extract::TarballExtractionMode;
use crate::NpmCache;
use crate::NpmCacheEnv;
use crate::NpmCacheSetting;

type LoadResult = Result<(), Arc<AnyError>>;
type LoadFuture = LocalBoxFuture<'static, LoadResult>;

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't finished yet.
  Pending(Arc<MultiRuntimeAsyncValueCreator<LoadResult>>),
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
pub struct TarballCache<TEnv: NpmCacheEnv> {
  cache: Arc<NpmCache<TEnv>>,
  env: Arc<TEnv>,
  npmrc: Arc<ResolvedNpmRc>,
  memory_cache: Mutex<HashMap<PackageNv, MemoryCacheItem>>,
}

impl<TEnv: NpmCacheEnv> TarballCache<TEnv> {
  pub fn new(
    cache: Arc<NpmCache<TEnv>>,
    env: Arc<TEnv>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache,
      env,
      npmrc,
      memory_cache: Default::default(),
    }
  }

  pub async fn ensure_package(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), AnyError> {
    self
      .ensure_package_inner(package_nv, dist)
      .await
      .with_context(|| format!("Failed caching npm package '{}'.", package_nv))
  }

  async fn ensure_package_inner(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), AnyError> {
    let cache_item = {
      let mut mem_cache = self.memory_cache.lock();
      if let Some(cache_item) = mem_cache.get(package_nv) {
        cache_item.clone()
      } else {
        let value_creator = MultiRuntimeAsyncValueCreator::new({
          let tarball_cache = self.clone();
          let package_nv = package_nv.clone();
          let dist = dist.clone();
          Box::new(move || {
            tarball_cache.create_setup_future(package_nv.clone(), dist.clone())
          })
        });
        let cache_item = MemoryCacheItem::Pending(Arc::new(value_creator));
        mem_cache.insert(package_nv.clone(), cache_item.clone());
        cache_item
      }
    };

    match cache_item {
      MemoryCacheItem::Cached => Ok(()),
      MemoryCacheItem::Errored(err) => Err(anyhow!("{:#}", err)),
      MemoryCacheItem::Pending(creator) => {
        let result = creator.get().await;
        match result {
          Ok(_) => {
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Cached;
            Ok(())
          }
          Err(err) => {
            let result_err = anyhow!("{:#}", err);
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Errored(err);
            Err(result_err)
          }
        }
      }
    }
  }

  fn create_setup_future(
    self: &Arc<Self>,
    package_nv: PackageNv,
    dist: NpmPackageVersionDistInfo,
  ) -> LoadFuture {
    let tarball_cache = self.clone();
    async move {
      let registry_url = tarball_cache.npmrc.get_registry_url(&package_nv.name);
      let package_folder =
        tarball_cache.cache.package_folder_for_nv_and_url(&package_nv, registry_url);
      let should_use_cache = tarball_cache.cache.should_use_cache_for_package(&package_nv);
      let package_folder_exists = tarball_cache.env.exists(&package_folder);
      if should_use_cache && package_folder_exists {
        return Ok(());
      } else if tarball_cache.cache.cache_setting() == &NpmCacheSetting::Only {
        return Err(deno_core::error::custom_error(
          "NotCached",
          format!(
            "npm package not found in cache: \"{}\", --cached-only is specified.",
            &package_nv.name
          )
        )
        );
      }

      if dist.tarball.is_empty() {
        bail!("Tarball URL was empty.");
      }

      // IMPORTANT: npm registries may specify tarball URLs at different URLS than the
      // registry, so we MUST get the auth for the tarball URL and not the registry URL.
      let tarball_uri = Url::parse(&dist.tarball)?;
      let maybe_registry_config =
        tarball_cache.npmrc.tarball_config(&tarball_uri);
      let maybe_auth_header = maybe_registry_config.and_then(|c| maybe_auth_header_for_npm_registry(c).ok()?);

      let result = tarball_cache.env
        .download_with_retries_on_any_tokio_runtime(tarball_uri, maybe_auth_header)
        .await;
      let maybe_bytes = match result {
        Ok(maybe_bytes) => maybe_bytes,
        Err(err) => {
          if err.status_code == Some(StatusCode::UNAUTHORIZED)
            && maybe_registry_config.is_none()
            && tarball_cache.npmrc.get_registry_config(&package_nv.name).auth_token.is_some()
          {
            bail!(
              concat!(
                "No auth for tarball URI, but present for scoped registry.\n\n",
                "Tarball URI: {}\n",
                "Scope URI: {}\n\n",
                "More info here: https://github.com/npm/cli/wiki/%22No-auth-for-URI,-but-auth-present-for-scoped-registry%22"
              ),
              dist.tarball,
              registry_url,
            )
          }
          return Err(err.into())
        },
      };
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
          deno_unsync::spawn_blocking(move || {
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
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }
}
