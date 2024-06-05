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
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::npm_rc::RegistryConfig;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;

use crate::args::CacheSetting;
use crate::http_util::HttpClientProvider;
use crate::npm::common::maybe_auth_header_for_npm_registry;
use crate::util::progress_bar::ProgressBar;

use super::NpmCache;

// todo(dsherret): create seams and unit test this

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't loaded yet.
  PendingFuture(Shared<PendingRegistryLoadFuture>),
  /// The item has loaded in the past and was stored in the file system cache.
  /// There is no reason to request this package from the npm registry again
  /// for the duration of execution.
  FsCached,
  /// An item is memory cached when it fails saving to the file system cache
  /// or the package does not exist.
  MemoryCached(Result<Option<Arc<NpmPackageInfo>>, Arc<AnyError>>),
}

#[derive(Debug, Clone)]
enum FutureResult {
  PackageNotExists,
  SavedFsCache(Arc<NpmPackageInfo>),
  ErroredFsCache(Arc<NpmPackageInfo>),
}

type PendingRegistryLoadFuture =
  BoxFuture<'static, Result<FutureResult, Arc<AnyError>>>;

/// Downloads packuments from the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct RegistryInfoDownloader {
  cache: Arc<NpmCache>,
  http_client_provider: Arc<HttpClientProvider>,
  npmrc: Arc<ResolvedNpmRc>,
  progress_bar: ProgressBar,
  memory_cache: Mutex<HashMap<String, MemoryCacheItem>>,
}

impl RegistryInfoDownloader {
  pub fn new(
    cache: Arc<NpmCache>,
    http_client_provider: Arc<HttpClientProvider>,
    npmrc: Arc<ResolvedNpmRc>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      cache,
      http_client_provider,
      npmrc,
      progress_bar,
      memory_cache: Default::default(),
    }
  }

  pub async fn load_package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    let registry_url = self.npmrc.get_registry_url(name);
    let registry_config = self.npmrc.get_registry_config(name);

    self
      .load_package_info_inner(name, registry_url, registry_config)
      .await
      .with_context(|| {
        format!(
          "Error getting response at {} for package \"{}\"",
          self.get_package_url(name, registry_url),
          name
        )
      })
  }

  async fn load_package_info_inner(
    &self,
    name: &str,
    registry_url: &Url,
    registry_config: &RegistryConfig,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    if *self.cache.cache_setting() == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{name}\", --cached-only is specified."
        )
      ));
    }

    let (created, cache_item) = {
      let mut mem_cache = self.memory_cache.lock();
      if let Some(cache_item) = mem_cache.get(name) {
        (false, cache_item.clone())
      } else {
        let future =
          self.create_load_future(name, registry_url, registry_config);
        let cache_item = MemoryCacheItem::PendingFuture(future);
        mem_cache.insert(name.to_string(), cache_item.clone());
        (true, cache_item)
      }
    };
    match cache_item {
      MemoryCacheItem::FsCached => {
        // this struct previously loaded from the registry, so we can load it from the file system cache
        self
          .load_file_cached_package_info(name)
          .await
          .map(|info| Some(Arc::new(info)))
      }
      MemoryCacheItem::MemoryCached(maybe_info) => {
        maybe_info.clone().map_err(|e| anyhow!("{}", e))
      }
      MemoryCacheItem::PendingFuture(future) => {
        if created {
          match future.await {
            Ok(FutureResult::SavedFsCache(info)) => {
              // return back the future and mark this package as having
              // been saved in the cache for next time it's requested
              *self.memory_cache.lock().get_mut(name).unwrap() =
                MemoryCacheItem::FsCached;
              Ok(Some(info))
            }
            Ok(FutureResult::ErroredFsCache(info)) => {
              // since saving to the fs cache failed, keep the package information in memory
              *self.memory_cache.lock().get_mut(name).unwrap() =
                MemoryCacheItem::MemoryCached(Ok(Some(info.clone())));
              Ok(Some(info))
            }
            Ok(FutureResult::PackageNotExists) => {
              *self.memory_cache.lock().get_mut(name).unwrap() =
                MemoryCacheItem::MemoryCached(Ok(None));
              Ok(None)
            }
            Err(err) => {
              let return_err = anyhow!("{}", err);
              *self.memory_cache.lock().get_mut(name).unwrap() =
                MemoryCacheItem::MemoryCached(Err(err));
              Err(return_err)
            }
          }
        } else {
          match future.await {
            Ok(FutureResult::SavedFsCache(info)) => Ok(Some(info)),
            Ok(FutureResult::ErroredFsCache(info)) => Ok(Some(info)),
            Ok(FutureResult::PackageNotExists) => Ok(None),
            Err(err) => Err(anyhow!("{}", err)),
          }
        }
      }
    }
  }

  async fn load_file_cached_package_info(
    &self,
    name: &str,
  ) -> Result<NpmPackageInfo, AnyError> {
    // this scenario failing should be exceptionally rare so let's
    // deal with improving it only when anyone runs into an issue
    let maybe_package_info = deno_core::unsync::spawn_blocking({
      let cache = self.cache.clone();
      let name = name.to_string();
      move || cache.load_package_info(&name)
    })
    .await
    .unwrap()
    .with_context(|| {
      format!(
        "Previously saved '{}' from the npm cache, but now it fails to load.",
        name
      )
    })?;
    match maybe_package_info {
      Some(package_info) => Ok(package_info),
      None => {
        bail!("The package '{}' previously saved its registry information to the file system cache, but that file no longer exists.", name)
      }
    }
  }

  fn create_load_future(
    &self,
    name: &str,
    registry_url: &Url,
    registry_config: &RegistryConfig,
  ) -> Shared<PendingRegistryLoadFuture> {
    let package_url = self.get_package_url(name, registry_url);
    let maybe_auth_header = maybe_auth_header_for_npm_registry(registry_config);
    let guard = self.progress_bar.update(package_url.as_str());
    let cache = self.cache.clone();
    let http_client_provider = self.http_client_provider.clone();
    let name = name.to_string();
    // force this future to be polled on the current runtime because it's not
    // safe to share `HttpClient`s across runtimes and because a restart of
    // npm resolution might cause this package not to be resolved again
    // causing the future to never be polled
    deno_core::unsync::spawn(async move {
      let maybe_bytes = http_client_provider
        .get_or_create()?
        .download_with_progress(package_url, maybe_auth_header, &guard)
        .await?;
      match maybe_bytes {
        Some(bytes) => {
          let future_result = deno_core::unsync::spawn_blocking(
            move || -> Result<FutureResult, AnyError> {
              let package_info = serde_json::from_slice(&bytes)?;
              match cache.save_package_info(&name, &package_info) {
                Ok(()) => {
                  Ok(FutureResult::SavedFsCache(Arc::new(package_info)))
                }
                Err(err) => {
                  log::debug!(
                    "Error saving package {} to cache: {:#}",
                    name,
                    err
                  );
                  Ok(FutureResult::ErroredFsCache(Arc::new(package_info)))
                }
              }
            },
          )
          .await??;
          Ok(future_result)
        }
        None => Ok(FutureResult::PackageNotExists),
      }
    })
    .map(|result| result.unwrap().map_err(Arc::new))
    .boxed()
    .shared()
  }

  fn get_package_url(&self, name: &str, registry_url: &Url) -> Url {
    // list of all characters used in npm packages:
    //  !, ', (, ), *, -, ., /, [0-9], @, [A-Za-z], _, ~
    const ASCII_SET: percent_encoding::AsciiSet =
      percent_encoding::NON_ALPHANUMERIC
        .remove(b'!')
        .remove(b'\'')
        .remove(b'(')
        .remove(b')')
        .remove(b'*')
        .remove(b'-')
        .remove(b'.')
        .remove(b'/')
        .remove(b'@')
        .remove(b'_')
        .remove(b'~');
    let name = percent_encoding::utf8_percent_encode(name, &ASCII_SET);
    registry_url.join(&name.to_string()).unwrap()
  }
}
