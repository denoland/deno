// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_semver::package::PackageNv;
use deno_unsync::sync::MultiRuntimeAsyncValueCreator;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use http::StatusCode;
use parking_lot::Mutex;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;
use url::Url;

use crate::remote::maybe_auth_header_for_npm_registry;
use crate::tarball_extract::verify_and_extract_tarball;
use crate::tarball_extract::TarballExtractionMode;
use crate::NpmCache;
use crate::NpmCacheHttpClient;
use crate::NpmCacheSetting;

type LoadResult = Result<(), Arc<JsErrorBox>>;
type LoadFuture = LocalBoxFuture<'static, LoadResult>;

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't finished yet.
  Pending(Arc<MultiRuntimeAsyncValueCreator<LoadResult>>),
  /// The result errored.
  Errored(Arc<JsErrorBox>),
  /// This package has already been cached.
  Cached,
}

/// Coordinates caching of tarballs being loaded from
/// the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct TarballCache<
  THttpClient: NpmCacheHttpClient,
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsRemoveFile
    + FsReadDir
    + FsRename
    + ThreadSleep
    + SystemRandom
    + Send
    + Sync
    + 'static,
> {
  cache: Arc<NpmCache<TSys>>,
  http_client: Arc<THttpClient>,
  sys: TSys,
  npmrc: Arc<ResolvedNpmRc>,
  memory_cache: Mutex<HashMap<PackageNv, MemoryCacheItem>>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Failed caching npm package '{package_nv}'")]
pub struct EnsurePackageError {
  package_nv: Box<PackageNv>,
  #[source]
  source: Arc<JsErrorBox>,
}
impl<
    THttpClient: NpmCacheHttpClient,
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsRemoveFile
      + FsReadDir
      + FsRename
      + ThreadSleep
      + SystemRandom
      + Send
      + Sync
      + 'static,
  > TarballCache<THttpClient, TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    http_client: Arc<THttpClient>,
    sys: TSys,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache,
      http_client,
      sys,
      npmrc,
      memory_cache: Default::default(),
    }
  }

  pub async fn ensure_package(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), EnsurePackageError> {
    self
      .ensure_package_inner(package_nv, dist)
      .await
      .map_err(|source| EnsurePackageError {
        package_nv: Box::new(package_nv.clone()),
        source,
      })
  }

  async fn ensure_package_inner(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), Arc<JsErrorBox>> {
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
      MemoryCacheItem::Errored(err) => Err(err),
      MemoryCacheItem::Pending(creator) => {
        let result = creator.get().await;
        match result {
          Ok(_) => {
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Cached;
            Ok(())
          }
          Err(err) => {
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Errored(err.clone());
            Err(err)
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
      let package_folder_exists = tarball_cache.sys.fs_exists_no_err(&package_folder);
      if should_use_cache && package_folder_exists {
        return Ok(());
      } else if tarball_cache.cache.cache_setting() == &NpmCacheSetting::Only {
        return Err(JsErrorBox::new(
          "NotCached",
          format!(
            "npm package not found in cache: \"{}\", --cached-only is specified.",
            &package_nv.name
          )
        )
        );
      }

      if dist.tarball.is_empty() {
        return Err(JsErrorBox::generic("Tarball URL was empty."));
      }

      // IMPORTANT: npm registries may specify tarball URLs at different URLS than the
      // registry, so we MUST get the auth for the tarball URL and not the registry URL.
      let tarball_uri = Url::parse(&dist.tarball).map_err(JsErrorBox::from_err)?;
      let maybe_registry_config =
        tarball_cache.npmrc.tarball_config(&tarball_uri);
      let maybe_auth_header = maybe_registry_config.and_then(|c| maybe_auth_header_for_npm_registry(c).ok()?);

      let result = tarball_cache.http_client
        .download_with_retries_on_any_tokio_runtime(tarball_uri, maybe_auth_header)
        .await;
      let maybe_bytes = match result {
        Ok(maybe_bytes) => maybe_bytes,
        Err(err) => {
          if err.status_code == Some(StatusCode::UNAUTHORIZED)
            && maybe_registry_config.is_none()
            && tarball_cache.npmrc.get_registry_config(&package_nv.name).auth_token.is_some()
          {
            return Err(JsErrorBox::generic(format!(
              concat!(
                "No auth for tarball URI, but present for scoped registry.\n\n",
                "Tarball URI: {}\n",
                "Scope URI: {}\n\n",
                "More info here: https://github.com/npm/cli/wiki/%22No-auth-for-URI,-but-auth-present-for-scoped-registry%22"
              ),
              dist.tarball,
              registry_url,
            )));
          }
          return Err(JsErrorBox::from_err(err))
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
          .await.map_err(JsErrorBox::from_err)?.map_err(JsErrorBox::from_err)
        }
        None => {
          Err(JsErrorBox::generic(format!("Could not find npm package tarball at: {}", dist.tarball)))
        }
      }
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }
}
